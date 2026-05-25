#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::*;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{DriveMode, Input, InputConfig, Pull};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, channel, timer};
use esp_hal::time::Rate;
use static_cell::StaticCell;

use {panic_rtt_target as _, rtt_target as _};

// 🟢 Declare concrete types for the static cells
static LEDC_CELL: StaticCell<Ledc<'static>> = StaticCell::new();
static TIMER_CELL: StaticCell<timer::Timer<'static, esp_hal::ledc::LowSpeed>> = StaticCell::new();

#[embassy_executor::task]
async fn switch_monitor_task(
    mut switch: Input<'static>,
    led_pwm: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
) {
    loop {
        switch.wait_for_low().await;
        switch.wait_for_high().await;
        Timer::after(Duration::from_millis(20)).await;

        if switch.is_high() {
            led_pwm.set_duty(75).unwrap();
            Timer::after(Duration::from_millis(1500)).await;
            led_pwm.set_duty(0).unwrap();
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let switch_input = Input::new(
        peripherals.GPIO4,
        InputConfig::default().with_pull(Pull::Up),
    );

    // 🟢 Explicitly initialize the static cells
    let ledc = LEDC_CELL.init(Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);

    let pwm_timer = TIMER_CELL.init(ledc.timer::<esp_hal::ledc::LowSpeed>(timer::Number::Timer0));

    pwm_timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(5),
        })
        .unwrap();

    let mut pwm_channel = ledc.channel(channel::Number::Channel0, peripherals.GPIO5);
    pwm_channel
        .configure(channel::config::Config {
            timer: pwm_timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    spawner.spawn(switch_monitor_task(switch_input, pwm_channel).unwrap());

    loop {
        Timer::after(Duration::from_secs(10)).await;
    }
}
