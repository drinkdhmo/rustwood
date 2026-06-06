use alloc::format;

use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use picoserve::extract::Form;
use picoserve::response::Redirect;
use picoserve::{Router, routing};

use crate::LedConfig;

type LedConfigMutex = Mutex<CriticalSectionRawMutex, LedConfig>;

#[derive(serde::Deserialize)]
struct FormData {
    motor_spare_throttle_percent: u16,
    motor_left_wheel_throttle_percent: u16,
    motor_right_wheel_throttle_percent: u16,
    motor_fan_throttle_percent: u16,
    motor_fan_idle_throttle_percent: u16,
    arm_wait_ms: u64,
    on_duration_ms: u64,
    on_delay_ms: u64,
}

pub const WEB_TASK_POOL_SIZE: usize = 2;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    task_id: usize,
    stack: Stack<'static>,
    config: &'static picoserve::Config,
    led_config: &'static LedConfigMutex,
) {
    let app = Router::new().route(
        "/",
        routing::get(async move || {
            let (motor_spare_throttle_percent, motor_left_wheel_throttle_percent, motor_right_wheel_throttle_percent, motor_fan_throttle_percent, motor_fan_idle_throttle_percent, arm_wait_ms, on_duration_ms, on_delay_ms) = {
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
            let html = format!(
                "<!DOCTYPE html><html><head>\
<meta charset=utf-8><title>rustwood</title></head><body>\
<h2>rustwood motor config</h2>\
<form method=POST action=/>\
<label>Spare motor throttle (0-100): <input type=number name=motor_spare_throttle_percent min=0 max=100 value={motor_spare_throttle_percent}></label><br><br>\
<label>Left motor throttle (0-100): <input type=number name=motor_left_wheel_throttle_percent min=0 max=100 value={motor_left_wheel_throttle_percent}></label><br><br>\
<label>Right motor throttle (0-100): <input type=number name=motor_right_wheel_throttle_percent min=0 max=100 value={motor_right_wheel_throttle_percent}></label><br><br>\
<label>Fan motor throttle (0-100): <input type=number name=motor_fan_throttle_percent min=0 max=100 value={motor_fan_throttle_percent}></label><br><br>\
<label>Fan motor idle throttle (0-100): <input type=number name=motor_fan_idle_throttle_percent min=0 max=100 value={motor_fan_idle_throttle_percent}></label><br><br>\
<label>Arm wait ms: <input type=number name=arm_wait_ms min=0 value={arm_wait_ms}></label><br><br>\
<label>On-duration ms: <input type=number name=on_duration_ms min=0 value={on_duration_ms}></label><br><br>\
<label>On-delay ms: <input type=number name=on_delay_ms min=0 value={on_delay_ms}></label><br><br>\
<button type=submit>Apply</button>\
</form></body></html>"
            );
            (("content-type", "text/html"), html)
        })
        .post(async move |Form(data): Form<FormData>| {
            let motor_spare_throttle_percent = data.motor_spare_throttle_percent.min(100);
            let motor_left_wheel_throttle_percent = data.motor_left_wheel_throttle_percent.min(100);
            let motor_right_wheel_throttle_percent = data.motor_right_wheel_throttle_percent.min(100);
            let motor_fan_throttle_percent = data.motor_fan_throttle_percent.min(100);
            {
                let mut c = led_config.lock().await;
                c.motor_spare_throttle_percent = motor_spare_throttle_percent;
                c.motor_left_wheel_throttle_percent = motor_left_wheel_throttle_percent;
                c.motor_right_wheel_throttle_percent = motor_right_wheel_throttle_percent;
                c.motor_fan_throttle_percent = motor_fan_throttle_percent;
                c.motor_fan_idle_throttle_percent = data.motor_fan_idle_throttle_percent;
                c.arm_wait_ms = data.arm_wait_ms;
                c.on_duration_ms = data.on_duration_ms;
                c.on_delay_ms = data.on_delay_ms;
            }
            defmt::info!(
                "web: throttle={}%,{}%,{}%,{}%,{}%, arm_wait={}ms, duration={}ms, delay={}ms",
                motor_spare_throttle_percent,
                motor_left_wheel_throttle_percent,
                motor_right_wheel_throttle_percent,
                motor_fan_throttle_percent,
                data.motor_fan_idle_throttle_percent,
                data.arm_wait_ms,
                data.on_duration_ms,
                data.on_delay_ms
            );
            Redirect::to("/")
        }),
    );

    let mut tcp_rx = [0u8; 1024];
    let mut tcp_tx = [0u8; 1024];
    let mut http_buf = [0u8; 2048];

    picoserve::Server::new(&app, config, &mut http_buf)
        .listen_and_serve(task_id, stack, 80, &mut tcp_rx, &mut tcp_tx)
        .await;
}
