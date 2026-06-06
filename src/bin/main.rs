#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use defmt::*;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Pull};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::ledc::channel::{ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{Ledc, channel, timer};
use esp_hal::rmt::{Channel, Rmt, Tx, TxChannelConfig, TxChannelCreator};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use static_cell::StaticCell;

use rustwood::{LedConfig, LedConfigMutex, RgbColor, neopixel::neopixel_frame, storage, web, wifi};

use defmt_rtt as _;
use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();

// 🟢 Declare concrete types for the static cells
static LEDC_CELL: StaticCell<Ledc<'static>> = StaticCell::new();
static TIMER_CELL: StaticCell<timer::Timer<'static, esp_hal::ledc::LowSpeed>> = StaticCell::new();
static FLASH_CELL: StaticCell<storage::FlashMutex> = StaticCell::new();

static WEB_CONFIG: picoserve::Config = picoserve::Config::const_default();
static LED_CONFIG_CELL: StaticCell<LedConfigMutex> = StaticCell::new();

const SERVO_PWM_FREQ_HZ: u32 = 50;
const SERVO_PERIOD_US: u32 = 1_000_000 / SERVO_PWM_FREQ_HZ;
const SERVO_MIN_PULSE_US: u32 = 1_000;
const SERVO_MAX_PULSE_US: u32 = 2_000;
const SERVO_IDLE_ANGLE_DEG: u16 = 0;
const SERVO_MAX_ANGLE_DEG: u16 = 180;
const SERVO_DUTY_BITS: u32 = 12;

struct ServoOutputs {
    servo1: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
    servo2: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
    servo3: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
    servo4: channel::Channel<'static, esp_hal::ledc::LowSpeed>,
}

impl ServoOutputs {
    fn set_angles(
        &self,
        servo1_angle_deg: u16,
        servo2_angle_deg: u16,
        servo3_angle_deg: u16,
        servo4_angle_deg: u16,
    ) {
        set_servo_angle(&self.servo1, servo1_angle_deg);
        set_servo_angle(&self.servo2, servo2_angle_deg);
        set_servo_angle(&self.servo3, servo3_angle_deg);
        set_servo_angle(&self.servo4, servo4_angle_deg);
    }

    fn set_idle(&self) {
        self.set_angles(
            SERVO_IDLE_ANGLE_DEG,
            SERVO_IDLE_ANGLE_DEG,
            SERVO_IDLE_ANGLE_DEG,
            SERVO_IDLE_ANGLE_DEG,
        );
    }
}

fn servo_pulse_us_from_angle(angle_deg: u16) -> u32 {
    let clamped = angle_deg.min(SERVO_MAX_ANGLE_DEG) as u32;
    let span = SERVO_MAX_PULSE_US - SERVO_MIN_PULSE_US;
    SERVO_MIN_PULSE_US
        + ((span * clamped + (SERVO_MAX_ANGLE_DEG as u32 / 2)) / SERVO_MAX_ANGLE_DEG as u32)
}

fn servo_duty_ticks_from_pulse_us(pulse_us: u32) -> u32 {
    let max_duty = 1u32 << SERVO_DUTY_BITS;
    ((pulse_us * max_duty) + (SERVO_PERIOD_US / 2)) / SERVO_PERIOD_US
}

fn set_servo_angle(servo_pwm: &channel::Channel<'static, esp_hal::ledc::LowSpeed>, angle_deg: u16) {
    let pulse_us = servo_pulse_us_from_angle(angle_deg);
    let duty_ticks = servo_duty_ticks_from_pulse_us(pulse_us);
    servo_pwm.set_duty_hw(duty_ticks);
}

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
    servo_outputs: ServoOutputs,
    neopixel_tx: Channel<'static, esp_hal::Blocking, Tx>,
    led_config: &'static LedConfigMutex,
) {
    let mut neopixel_tx = neopixel_tx;

    loop {
        // Idle state: blue.
        neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::blue(50));

        switch.wait_for_low().await;
        // debounce
        Timer::after(Duration::from_millis(20)).await;
        if switch.is_high() {
            continue;
        }

        // Closed and arming state: orange.
        neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::orange(50));
        let (
            motor_spare_throttle_percent,
            motor_left_wheel_throttle_percent,
            motor_right_wheel_throttle_percent,
            motor_fan_throttle_percent,
            motor_fan_idle_throttle_percent,
            arm_wait_ms,
            on_duration_ms,
            on_delay_ms,
        ) = {
            let c = led_config.lock().await;
            (
                c.motor_spare_throttle_percent,
                c.motor_left_wheel_throttle_percent,
                c.motor_right_wheel_throttle_percent,
                c.motor_fan_throttle_percent,
                c.motor_fan_idle_throttle_percent,
                c.arm_wait_ms,
                c.on_duration_ms,
                c.on_delay_ms,
            )
        };
        Timer::after(Duration::from_millis(arm_wait_ms)).await;
        if switch.is_high() {
            // If the switch was released during the arming delay, skip activation and return to idle.
            continue;
        }

        // Closed and armed state: red.
        neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::red(50));

        {
            let servo1_angle_deg = 0;
            let servo2_angle_deg = 0;
            let servo3_angle_deg = 0;
            let servo4_angle_deg = (motor_fan_idle_throttle_percent * SERVO_MAX_ANGLE_DEG) / 100;

            servo_outputs.set_angles(
                servo1_angle_deg,
                servo2_angle_deg,
                servo3_angle_deg,
                servo4_angle_deg,
            );
        }

        switch.wait_for_high().await;
        Timer::after(Duration::from_millis(20)).await;

        if switch.is_high() {
            // println!(
            //     "Switch released - throttle={}%,{}%,{}%,{}%,{}%, arm_wait={}ms, duration={}ms, delay={}ms",
            //     motor_spare_throttle_percent,
            //     motor_left_wheel_throttle_percent,
            //     motor_right_wheel_throttle_percent,
            //     motor_fan_throttle_percent,
            //     motor_fan_idle_throttle_percent,
            //     arm_wait_ms,
            //     on_duration_ms,
            //     on_delay_ms
            // );

            let servo1_angle_deg = (motor_spare_throttle_percent * SERVO_MAX_ANGLE_DEG) / 100;
            let servo2_angle_deg = (motor_left_wheel_throttle_percent * SERVO_MAX_ANGLE_DEG) / 100;
            let servo3_angle_deg = (motor_right_wheel_throttle_percent * SERVO_MAX_ANGLE_DEG) / 100;
            let servo4_angle_deg = (motor_fan_throttle_percent * SERVO_MAX_ANGLE_DEG) / 100;
            // Active delay state: yellow
            neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::yellow(50));
            Timer::after(Duration::from_millis(on_delay_ms)).await;

            servo_outputs.set_angles(
                servo1_angle_deg,
                servo2_angle_deg,
                servo3_angle_deg,
                servo4_angle_deg,
            );

            // Active run state: green
            neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::green(50));
            Timer::after(Duration::from_millis(on_duration_ms)).await;
            servo_outputs.set_idle();

            neopixel_tx = set_neopixel_color(neopixel_tx, RgbColor::blue(50));
            println!("Servos returned to idle angle");
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let flash_storage = FLASH_CELL.init(storage::FlashMutex::new(esp_storage::FlashStorage::new(
        peripherals.FLASH,
    )));
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    // Embassy runtime timing is driven from TIMG0 timer0.
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

    // Servo PWM uses LEDC timer1, a different peripheral timer domain from TIMG0.
    let pwm_timer = TIMER_CELL.init(ledc.timer::<esp_hal::ledc::LowSpeed>(timer::Number::Timer1));

    pwm_timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty12Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(SERVO_PWM_FREQ_HZ),
        })
        .unwrap();

    let mut servo1 = ledc.channel(channel::Number::Channel0, peripherals.GPIO5);
    servo1
        .configure(channel::config::Config {
            timer: pwm_timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();
    let mut servo2 = ledc.channel(channel::Number::Channel1, peripherals.GPIO6);
    servo2
        .configure(channel::config::Config {
            timer: pwm_timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let mut servo3 = ledc.channel(channel::Number::Channel2, peripherals.GPIO7);
    servo3
        .configure(channel::config::Config {
            timer: pwm_timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let mut servo4 = ledc.channel(channel::Number::Channel3, peripherals.GPIO15);
    servo4
        .configure(channel::config::Config {
            timer: pwm_timer,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let servo_outputs = ServoOutputs {
        servo1,
        servo2,
        servo3,
        servo4,
    };
    servo_outputs.set_idle();

    println!("tick");
    error!("error");
    warn!("warn");
    info!("info");
    debug!("debug");
    trace!("trace");
    Timer::after(Duration::from_millis(150)).await;

    println!("Starting switch monitor task...");
    info!("Starting switch monitor task...");
    let led_config_value = match storage::load_led_config(flash_storage).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("No persisted config found, using defaults");
            LedConfig::default()
        }
        Err(err) => {
            println!("Failed to load persisted config, using defaults: {:?}", err);
            LedConfig::default()
        }
    };
    let led_config = LED_CONFIG_CELL.init(LedConfigMutex::new(led_config_value));
    spawner
        .spawn(switch_monitor_task(switch_input, servo_outputs, neopixel_tx, led_config).unwrap());

    let (_wifi_controller, stack) = wifi::start_ap(peripherals.WIFI, &spawner).await;
    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner
            .spawn(web::web_task(id, stack, &WEB_CONFIG, flash_storage, led_config).unwrap());
    }

    loop {
        Timer::after(Duration::from_millis(10)).await;
    }
}
