#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{Level, Output},
    prelude::*,
    timer::timg::TimerGroup,
};
use log::info;

extern crate alloc;

#[main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    // so it's fixed size allocator
    esp_alloc::heap_allocator!(72 * 1024);

    // init logger
    esp_println::logger::init_logger_from_env();

    let timer0 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    info!("Embassy initialized!");

    // let timer1 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    // let _init = esp_wifi::init(
    //     timer1.timer0,
    //     esp_hal::rng::Rng::new(peripherals.RNG),
    //     peripherals.RADIO_CLK,
    // )
    // .unwrap();

    // TODO: Spawn some tasks
    let _ = spawner;

    let mut led = Output::new(peripherals.GPIO2, Level::Low);
    led.set_high();

    info!("Blinking example!");

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
}
