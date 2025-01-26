#![no_std]
#![no_main]
// for more heap
#![allow(static_mut_refs)]

use core::str::FromStr;

use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    DhcpConfig, Runner, StackResources,
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, gpio};
use esp_mbedtls::Tls;
use esp_wifi::{
    wifi::{self, WifiDevice, WifiStaDevice},
    EspWifiController,
};
use load_dotenv::load_dotenv;
use reqwless::client::TlsConfig;
use telegram::SendMessageError;

mod telegram;
extern crate alloc;

load_dotenv!();

const BOT_TOKEN: &str = env!("BOT_TOKEN");
const CHAT_ID: &str = env!("CHAT_ID");

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

fn init_heap() {
    static mut HEAP: core::mem::MaybeUninit<[u8; 80 * 1024]> = core::mem::MaybeUninit::uninit();

    #[link_section = ".dram2_uninit"]
    static mut HEAP2: core::mem::MaybeUninit<[u8; 64 * 1024]> = core::mem::MaybeUninit::uninit();

    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            HEAP.as_mut_ptr() as *mut u8,
            core::mem::size_of_val(&*core::ptr::addr_of!(HEAP)),
            esp_alloc::MemoryCapability::Internal.into(),
        ));

        // COEX needs more RAM - add some more
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            HEAP2.as_mut_ptr() as *mut u8,
            core::mem::size_of_val(&*core::ptr::addr_of!(HEAP2)),
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.2.2

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    init_heap();

    esp_println::logger::init_logger_from_env();

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    log::info!("Embassy initialized!");

    let mut rng = esp_hal::rng::Rng::new(peripherals.RNG);

    let timer1 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    let wifi_init = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng, peripherals.RADIO_CLK).expect("esp_wifi::init failed")
    );

    log::info!("Wifi initialized!");

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) = wifi::new_with_mode(wifi_init, wifi, WifiStaDevice).unwrap();

    log::info!("Wifi is setup!");

    let net_config = embassy_net::Config::dhcpv4(DhcpConfig::default());

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        random_u64(&mut rng),
    );

    log::info!("Spawning tasks to connect");
    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    stack.wait_link_up().await;
    log::info!("WIFI: Waiting to get IP address...");
    stack.wait_config_up().await;
    let ip = alloc::format!("{}", stack.config_v4().expect("stack config up").address);
    log::info!("WIFI: Got IP {}", ip);

    let dns = DnsSocket::new(stack);

    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = Tls::new(peripherals.SHA)
        .expect("TLS::new with peripherals.SHA failed")
        .with_hardware_rsa(peripherals.RSA);

    let tls_config = TlsConfig::new(
        reqwless::TlsVersion::Tls1_3,
        reqwless::Certificates {
            ca_chain: reqwless::X509::pem(concat!(include_str!("./cert.pem"), "\0").as_bytes())
                .ok(),
            ..Default::default()
        },
        tls.reference(),
    );

    // let mut client = HttpClient::new_with_tls(&tcp, &dns, tls_config);

    // const MAX_ATTEMPTS: u8 = 7;
    // let connection = {
    //     let mut attempt = 0;
    //     loop {
    //         attempt += 1;
    //         log::info!("HTTP: Trying to connect to Telegram, attempt {}", attempt);
    //         match client.resource(telegram::HOSTNAME).await {
    //             Ok(value) => break Ok(value),
    //             Err(_) if attempt < MAX_ATTEMPTS => {
    //                 log::warn!("HTTP: Connection attempt {} failed. Retrying", attempt);
    //                 Timer::after_millis(50).await
    //             }
    //             Err(err) => break Err(err),
    //         }
    //     }
    // }
    // .expect("HTTP: Connection to Telegram failed");
    // log::info!("HTTP: Connection is established");

    let mut tg = telegram::Client::new(&tcp, &dns, tls_config, BOT_TOKEN, CHAT_ID);

    let mut led = gpio::Output::new(peripherals.GPIO2, gpio::Level::Low);

    let mut offset: i64 = 0;
    let mut updates_failed_in_row = 0;
    loop {
        if updates_failed_in_row > 10 {
            break;
        }
        Timer::after_millis(500).await;

        // log::info!("MAIN LOOP: Getting updates");
        let updates = match tg.get_updates(offset).await {
            Ok(value) => {
                updates_failed_in_row = 0;
                value
            }
            Err(_) => {
                log::error!("MAIN LOOP: Can't get updates");
                updates_failed_in_row += 1;
                continue;
            }
        };

        for update in updates.result {
            if let Some(message) = update.message {
                if let Some(command) = message.text.strip_prefix('/') {
                    if command == "led" {
                        log::info!("COMMAND: /led");

                        led.toggle();
                    } else if command == "ip" {
                        log::info!("COMMAND: /ip");

                        if let Err(err) = tg.send_message(&alloc::format!("IP: <span class='tg-spoiler'>{}</span>", ip), true).await {
                            match err {
                                SendMessageError::TooSmallBodyBuffer => {
                                    log::error!("send_message buffer should be increased")
                                }
                                SendMessageError::StatusCodeIsNotSuccessful(status) => {
                                        log::error!("Status code isn't successful: {}", status.0);

                                }
                                SendMessageError::ReqwlessError(err) => {
                                    if let reqwless::Error::ConnectionAborted = err {
                                        log::error!("We need to reconnect");
                                    }
                                }
                            }
                        };
                    } else if let Some(content) = command.strip_prefix("echo") {
                        log::info!("COMMAND: /echo");

                        if let Err(err) = tg.send_message(content, false).await {
                            match err {
                                SendMessageError::TooSmallBodyBuffer => {
                                    log::error!("send_message buffer should be increased")
                                }
                      SendMessageError::StatusCodeIsNotSuccessful(status) => {
                                        log::error!("Status code isn't successful: {}", status.0);

                                }
                                SendMessageError::ReqwlessError(err) => {
                                    if let reqwless::Error::ConnectionAborted = err {
                                        log::error!("We need to reconnect");
                                    }
                                }
                            }
                        }
                    }
                }
            }

            offset = update.update_id + 1;
            blink(&mut led).await;
        }
    }
}

async fn blink(output: &mut gpio::Output<'_>) {
    let level = output.output_level();

    output.set_low();
    Timer::after_millis(50).await;
    output.set_high();
    Timer::after_millis(100).await;
    output.set_low();
    Timer::after_millis(50).await;

    output.set_level(level);
}

/**
The connection_task function manages the Wi-Fi connection by continuously
checking the status, configuring the Wi-Fi controller, and attempting to
reconnect if the connection is lost or not started.
*/
#[embassy_executor::task]
async fn connection_task(mut controller: wifi::WifiController<'static>) {
    log::info!("WIFI: Start connection task");
    loop {
        if wifi::wifi_state() == wifi::WifiState::StaConnected {
            // wait until we're no longer connected
            controller
                .wait_for_event(wifi::WifiEvent::StaDisconnected)
                .await;
            log::warn!("WIFI: Disconnected");
            Timer::after(Duration::from_millis(500)).await
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let ssid = heapless::String::<32>::from_str(env!("SSID"))
                .expect("Can't turn SSID into String<32>");
            let password = heapless::String::<64>::from_str(env!("PASSWORD"))
                .expect("Can't turn PASSWORD into String<64>");

            let client_config = wifi::Configuration::Client(wifi::ClientConfiguration {
                ssid,
                password,
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            controller.start_async().await.unwrap();
            log::info!("WIFI: Started!");
        }

        log::info!("WIFI: About to connect...");
        match controller.connect_async().await {
            Ok(_) => log::info!("WIFI: Wifi connected!"),
            Err(e) => {
                log::warn!("WIFI: Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(500)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

fn random_u64(rng: &mut esp_hal::rng::Rng) -> u64 {
    rng.random() as u64 | ((rng.random() as u64) << 32)
}
