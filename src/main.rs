#![no_std]
#![no_main]

mod telegram;

use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    DhcpConfig, Stack, StackResources,
};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{prelude::*, rng::Rng, timer::timg::TimerGroup};
use esp_println::println;
use esp_wifi::{
    wifi::{
        self, ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent,
        WifiStaDevice, WifiState,
    },
    EspWifiController,
};
// use heapless::String;
use load_dotenv::load_dotenv;
use log::info;
use reqwless::{
    client::{HttpClient, TlsConfig},
    headers::ContentType,
    request::RequestBuilder,
};
use nourl::{Url, UrlScheme};

extern crate alloc;

load_dotenv!();

const SSID: &'static str = env!("SSID");
const PASSWORD: &'static str = env!("PASSWORD");
const BOT_TOKEN: &'static str = env!("BOT_TOKEN");
const CHAT_ID: &'static str = env!("CHAT_ID");

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    // init logger
    esp_println::logger::init_logger_from_env();

    // so it's fixed size allocator
    esp_alloc::heap_allocator!(72 * 1024);

    // timer group is like hmmm
    // ?????
    let timer_group = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timer_group.timer0);

    info!("Embassy initialized!");

    let mut rng = Rng::new(peripherals.RNG);
    let net_seed = random_u64(&mut rng);
    let tls_seed = random_u64(&mut rng);

    // Initializing the Wi-Fi Controller
    let timer_group = TimerGroup::new(peripherals.TIMG1);

    let wifi_init = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer_group.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap()
    );

    let mut wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&wifi_init, wifi, WifiStaDevice).unwrap();

    let dhcp_config = DhcpConfig::default();
    
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    let stack = &*mk_static!(
        Stack<WifiDevice<'_, WifiStaDevice>>,
        Stack::new(
            wifi_interface,
            net_config,
            mk_static!(StackResources<3>, StackResources::<3>::new()),
            net_seed
        )
    );

    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(stack)).ok();

    println!("Waiting to link stack...");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    access_website(stack, tls_seed).await;
}

async fn access_website(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>, tls_seed: u64) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(&stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 4096];

    info!("Preparing sending Telegram message");

    let url = telegram::send_message_url(&BOT_TOKEN);
    let body = telegram::send_message_body(&CHAT_ID, "hi from esp32", false);

    info!("Forming request");

    let mut request = client
        .request(reqwless::request::Method::POST, &url)
        .await
        .unwrap()
        .body(body.as_bytes())
        .content_type(ContentType::ApplicationJson);

    info!("Making request");

    let response = request.send(&mut buffer).await.unwrap();

    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    println!("{}", content);
}

/**
The connection_task function manages the Wi-Fi connection by continuously
checking the status, configuring the Wi-Fi controller, and attempting to
reconnect if the connection is lost or not started.
*/
#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    println!("Start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());

    loop {
        match wifi::wifi_state() {
            wifi::WifiState::StaConnected => {
                // wait until we're no longer connected
                controller
                    .wait_for_event(wifi::WifiEvent::StaDisconnected)
                    .await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = wifi::Configuration::Client(wifi::ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}

/**
We need a random number for both the TLS configuration and network stack
initialization, and both require a u64. However, since rng generates only u32
values, we generate two random numbers and place one in the most significant
bits (MSB) and the other in the least significant bits (LSB) using bitwise operation
*/
fn random_u64(rng: &mut Rng) -> u64 {
    rng.random() as u64 | ((rng.random() as u64) << 32)
}

/*

let mut led = gpio::Output::new(peripherals.GPIO2, gpio::Level::Low);
    led.set_high();

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(150)).await;
        led.set_low();
        Timer::after(Duration::from_millis(150)).await;
        led.set_high();
        Timer::after(Duration::from_millis(150)).await;
        led.set_low();
        Timer::after(Duration::from_secs(5)).await;
    }

*/
