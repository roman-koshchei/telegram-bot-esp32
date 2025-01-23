#![no_std]
#![no_main]

use core::str::FromStr;

use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    DhcpConfig, Runner, Stack, StackResources,
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_mbedtls::Tls;
use esp_wifi::{
    wifi::{self, WifiDevice, WifiStaDevice},
    EspWifiController,
};
use load_dotenv::load_dotenv;
use log::info;
use reqwless::{
    client::{HttpClient, TlsConfig},
    headers::ContentType,
    request::RequestBuilder,
};

mod telegram;
extern crate alloc;

load_dotenv!();

const BOT_TOKEN: &str = env!("BOT_TOKEN");
const CHAT_ID: &str = env!("CHAT_ID");
const CERT: &[u8] = include_bytes!("./cert.pem"); // env!("CERT");

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // generator version: 0.2.2

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(112 * 1024);

    esp_println::logger::init_logger_from_env();

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    let mut rng = esp_hal::rng::Rng::new(peripherals.RNG);
    let net_seed = random_u64(&mut rng);

    let timer1 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    let wifi_init = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng, peripherals.RADIO_CLK).unwrap()
    );

    info!("Wifi initialized!");

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(wifi_init, wifi, WifiStaDevice).unwrap();

    info!("Wifi is setup!");

    let dhcp_config = DhcpConfig::default();
    let net_config = embassy_net::Config::dhcpv4(dhcp_config);

    let (stack, runner) = mk_static!(
        (
            embassy_net::Stack<'_>,
            Runner<'_, WifiDevice<'_, WifiStaDevice>>
        ),
        embassy_net::new(
            wifi_interface,
            net_config,
            mk_static!(StackResources<4>, StackResources::<4>::new()),
            net_seed
        )
    );

    info!("Spawning tasks to connect");
    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    info!("Waiting to link stack...");
    stack.wait_link_up().await;

    info!("Waiting to get IP address...");
    stack.wait_config_up().await;

    let tls = Tls::new(peripherals.SHA)
        .expect("TLS::new with peripherals.SHA failed")
        .with_hardware_rsa(peripherals.RSA);

    access_website(stack, tls).await;
}

async fn access_website(stack: &Stack<'_>, tls: Tls<'_>) {
    let dns = DnsSocket::new(*stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(*stack, &tcp_state);

    let tls = TlsConfig::new(
        reqwless::TlsVersion::Tls1_3,
        reqwless::Certificates {
            ca_chain: reqwless::X509::pem(CERT).ok(),
            ..Default::default()
        },
        tls.reference(),
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 4096];

    info!("Preparing sending Telegram message");

    let url = telegram::send_message_url(BOT_TOKEN);
    let body = telegram::send_message_body(CHAT_ID, "hi from esp32", false);

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
    info!("{}", content);
}

/**
The connection_task function manages the Wi-Fi connection by continuously
checking the status, configuring the Wi-Fi controller, and attempting to
reconnect if the connection is lost or not started.
*/
#[embassy_executor::task]
async fn connection_task(mut controller: wifi::WifiController<'static>) {
    info!("Start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());

    loop {
        if wifi::wifi_state() == wifi::WifiState::StaConnected {
            // wait until we're no longer connected
            controller
                .wait_for_event(wifi::WifiEvent::StaDisconnected)
                .await;
            Timer::after(Duration::from_millis(5000)).await
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
            info!("Starting wifi");
            controller.start_async().await.unwrap();
            info!("Wifi started!");
        }
        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => info!("Wifi connected!"),
            Err(e) => {
                info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(runner: &'static mut Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

fn random_u64(rng: &mut esp_hal::rng::Rng) -> u64 {
    rng.random() as u64 | ((rng.random() as u64) << 32)
}
