#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::*;
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, channel, timer};
use esp_hal::time::{Duration, Instant, Rate};
use esp_println::println;
use static_cell::StaticCell;

use defmt_rtt as _;
use esp_backtrace as _;

// 🟢 Declare concrete types for the static cells
static LEDC_CELL: StaticCell<Ledc<'static>> = StaticCell::new();
static TIMER_CELL: StaticCell<timer::Timer<'static, esp_hal::ledc::LowSpeed>> = StaticCell::new();

#[esp_rtos::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    println!("start");
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

    let mut led_dig = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());

    println!("tick");
    error!("error");
    warn!("warn");
    info!("info");
    debug!("debug");
    trace!("trace");
    led_dig.set_low();

    let mut last_tick = Instant::now();
    let mut last_switch_high = switch_input.is_high();
    let mut release_candidate_at: Option<Instant> = None;
    let mut led_on_at: Option<Instant> = None;

    loop {
        if last_tick.elapsed() >= Duration::from_secs(1) {
            println!("tick");
            last_tick = Instant::now();
        }

        let switch_high = switch_input.is_high();

        if !last_switch_high && switch_high {
            release_candidate_at = Some(Instant::now());
        }

        if !switch_high {
            release_candidate_at = None;
        }

        if let Some(candidate_at) = release_candidate_at {
            if switch_high && candidate_at.elapsed() >= Duration::from_millis(20) {
                println!("Switch released - turning on LED for 1.5s");
                led_dig.set_high();
                pwm_channel.set_duty(75).unwrap();
                led_on_at = Some(Instant::now());
                release_candidate_at = None;
            }
        }

        if let Some(started_at) = led_on_at {
            if started_at.elapsed() >= Duration::from_millis(1500) {
                pwm_channel.set_duty(0).unwrap();
                led_dig.set_low();
                println!("LED off");
                led_on_at = None;
            }
        }

        last_switch_high = switch_high;
    }
}
