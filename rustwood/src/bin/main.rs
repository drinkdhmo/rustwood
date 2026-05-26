#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::*;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, channel, timer};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use static_cell::StaticCell;

use rustwood::{LedConfig, LedConfigMutex, web, wifi};

use defmt_rtt as _;
use esp_backtrace as _;

// 🟢 Declare concrete types for the static cells
static LEDC_CELL: StaticCell<Ledc<'static>> = StaticCell::new();
static TIMER_CELL: StaticCell<timer::Timer<'static, esp_hal::ledc::LowSpeed>> = StaticCell::new();

static WEB_CONFIG: picoserve::Config = picoserve::Config::const_default();
static LED_CONFIG_CELL: StaticCell<LedConfigMutex> = StaticCell::new();

#[embassy_executor::task]
async fn switch_monitor_task(
    mut switch: Input<'static>,
    led_pwm: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
    mut led_dig: Output<'static>,
    led_config: &'static LedConfigMutex,
) {
    loop {
        switch.wait_for_low().await;
        switch.wait_for_high().await;
        Timer::after(Duration::from_millis(20)).await;

        if switch.is_high() {
            let (duty, delay_ms) = {
                let c = led_config.lock().await;
                (c.duty_pct, c.on_delay_ms)
            };
            println!("Switch released - duty={}% delay={}ms", duty, delay_ms);
            led_dig.set_high();
            led_pwm.set_duty(duty).unwrap();
            Timer::after(Duration::from_millis(delay_ms)).await;
            led_pwm.set_duty(0).unwrap();
            led_dig.set_low();
            println!("LED off");
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

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
    Timer::after(Duration::from_millis(150)).await;
    // panic!("BOOT CHECK: code is running!");
    led_dig.set_low();

    println!("Starting switch monitor task...");
    info!("Starting switch monitor task...");
    let led_config = LED_CONFIG_CELL.init(LedConfigMutex::new(LedConfig::default()));
    spawner.spawn(switch_monitor_task(switch_input, pwm_channel, led_dig, led_config).unwrap());

    let (_wifi_controller, stack) = wifi::start_ap(peripherals.WIFI, &spawner).await;
    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner.spawn(web::web_task(id, stack, &WEB_CONFIG, led_config).unwrap());
    }

    loop {
        println!("tick");
        Timer::after(Duration::from_secs(1)).await;
    }
}
