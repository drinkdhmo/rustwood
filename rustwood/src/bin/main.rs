#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::*;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Pull};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, channel, timer};
use esp_hal::rmt::{Channel, Rmt, Tx, TxChannelConfig, TxChannelCreator};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use static_cell::StaticCell;

use rustwood::{LedConfig, LedConfigMutex, RgbColor, neopixel::neopixel_frame, web, wifi};

use defmt_rtt as _;
use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();

// 🟢 Declare concrete types for the static cells
static LEDC_CELL: StaticCell<Ledc<'static>> = StaticCell::new();
static TIMER_CELL: StaticCell<timer::Timer<'static, esp_hal::ledc::LowSpeed>> = StaticCell::new();

static WEB_CONFIG: picoserve::Config = picoserve::Config::const_default();
static LED_CONFIG_CELL: StaticCell<LedConfigMutex> = StaticCell::new();

fn set_neopixel_color(
    channel: Channel<'static, esp_hal::Blocking, Tx>,
    color: RgbColor,
) -> Channel<'static, esp_hal::Blocking, Tx> {
    let frame = neopixel_frame(color);
    let tx = channel
        .transmit(&frame)
        .unwrap_or_else(|(err, _)| ::core::panic!("NeoPixel transmit failed: {:?}", err));

    tx.wait()
        .unwrap_or_else(|(err, _)| ::core::panic!("NeoPixel wait failed: {:?}", err))
}

#[embassy_executor::task]
async fn switch_monitor_task(
    mut switch: Input<'static>,
    led_pwm: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
    neopixel_tx: Channel<'static, esp_hal::Blocking, Tx>,
    led_config: &'static LedConfigMutex,
) {
    let mut neopixel_tx = neopixel_tx;

    loop {
        // Idle state: blue.
        neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::blue(50));

        switch.wait_for_low().await;

        // Closed and armed state: red.
        neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::red(50));

        switch.wait_for_high().await;
        Timer::after(Duration::from_millis(20)).await;

        if switch.is_high() {
            let (duty, delay_ms) = {
                let c = led_config.lock().await;
                (c.duty_pct, c.on_delay_ms)
            };
            println!("Switch released - duty={}% delay={}ms", duty, delay_ms);

            // Active timeout state: green, using the general RGB helper path.
            neopixel_tx =
                set_neopixel_color(neopixel_tx, RgbColor::rgb_with_brightness(0, 255, 0, 50));

            led_pwm.set_duty(duty).unwrap();
            Timer::after(Duration::from_millis(delay_ms)).await;
            led_pwm.set_duty(0).unwrap();

            neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::blue(50));
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

    // Phase 1 RGB wiring: reserve an RMT TX channel for the built-in NeoPixel on GPIO48.
    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80)).expect("Failed to initialize RMT");
    let neopixel_tx_config = TxChannelConfig::default()
        .with_clk_divider(1)
        .with_idle_output_level(Level::Low)
        .with_idle_output(true)
        .with_carrier_modulation(false);
    let neopixel_tx = rmt
        .channel0
        .configure_tx(&neopixel_tx_config)
        .expect("Failed to configure RMT TX channel for NeoPixel")
        .with_pin(peripherals.GPIO48);
    let neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::blue(50));

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

    println!("tick");
    error!("error");
    warn!("warn");
    info!("info");
    debug!("debug");
    trace!("trace");
    Timer::after(Duration::from_millis(150)).await;

    println!("Starting switch monitor task...");
    info!("Starting switch monitor task...");
    let led_config = LED_CONFIG_CELL.init(LedConfigMutex::new(LedConfig::default()));
    spawner.spawn(switch_monitor_task(switch_input, pwm_channel, neopixel_tx, led_config).unwrap());

    let (_wifi_controller, stack) = wifi::start_ap(peripherals.WIFI, &spawner).await;
    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner.spawn(web::web_task(id, stack, &WEB_CONFIG, led_config).unwrap());
    }

    loop {
        println!("tick");
        Timer::after(Duration::from_secs(1)).await;
    }
}
