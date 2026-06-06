use alloc::{format, string::String};

use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use picoserve::extract::Form;
use picoserve::{Router, routing};

use crate::{RustwoodConfig, storage};

type RustwoodConfigMutex = Mutex<CriticalSectionRawMutex, RustwoodConfig>;

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FormAction {
    Apply,
    Save,
}

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
    action: FormAction,
}

pub const WEB_TASK_POOL_SIZE: usize = 2;

fn build_rustwood_config(data: &FormData) -> RustwoodConfig {
    RustwoodConfig {
        motor_spare_throttle_percent: data.motor_spare_throttle_percent.min(100),
        motor_left_wheel_throttle_percent: data.motor_left_wheel_throttle_percent.min(100),
        motor_right_wheel_throttle_percent: data.motor_right_wheel_throttle_percent.min(100),
        motor_fan_throttle_percent: data.motor_fan_throttle_percent.min(100),
        motor_fan_idle_throttle_percent: data.motor_fan_idle_throttle_percent.min(100),
        arm_wait_ms: data.arm_wait_ms,
        on_duration_ms: data.on_duration_ms,
        on_delay_ms: data.on_delay_ms,
    }
}

fn render_page(config: &RustwoodConfig, message: Option<&str>) -> String {
    let message_html = message
        .map(|message| format!("<p style=\"color:#b71c1c;font-weight:600;\">{message}</p>"))
        .unwrap_or_default();

    format!(
        "<!DOCTYPE html><html><head>\
<meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>rustwood</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:42rem;margin:2rem auto;padding:0 1rem;line-height:1.4}}label{{display:block;margin:0.75rem 0}}input{{width:8rem;padding:0.3rem}}button{{margin-right:0.75rem;margin-top:1rem;padding:0.5rem 0.9rem}}.card{{border:1px solid #ccc;border-radius:12px;padding:1rem 1.2rem;box-shadow:0 2px 10px rgba(0,0,0,0.04)}}</style>\
</head><body><div class=card>\
<h2>rustwood motor config</h2>\
<p>Apply updates live settings. Save writes the current form values to flash so they survive reboot.</p>\
{message_html}\
<form method=POST action=/>\
<label>Spare motor throttle (0-100): <input type=number name=motor_spare_throttle_percent min=0 max=100 value={motor_spare_throttle_percent}></label>\
<label>Left motor throttle (0-100): <input type=number name=motor_left_wheel_throttle_percent min=0 max=100 value={motor_left_wheel_throttle_percent}></label>\
<label>Right motor throttle (0-100): <input type=number name=motor_right_wheel_throttle_percent min=0 max=100 value={motor_right_wheel_throttle_percent}></label>\
<label>Fan motor throttle (0-100): <input type=number name=motor_fan_throttle_percent min=0 max=100 value={motor_fan_throttle_percent}></label>\
<label>Fan motor idle throttle (0-100): <input type=number name=motor_fan_idle_throttle_percent min=0 max=100 value={motor_fan_idle_throttle_percent}></label>\
<label>Arm wait ms: <input type=number name=arm_wait_ms min=0 value={arm_wait_ms}></label>\
<label>On-duration ms: <input type=number name=on_duration_ms min=0 value={on_duration_ms}></label>\
<label>On-delay ms: <input type=number name=on_delay_ms min=0 value={on_delay_ms}></label>\
<button type=submit name=action value=apply>Apply</button>\
<button type=submit name=action value=save>Save to flash</button>\
</form></div></body></html>",
        message_html = message_html,
        motor_spare_throttle_percent = config.motor_spare_throttle_percent,
        motor_left_wheel_throttle_percent = config.motor_left_wheel_throttle_percent,
        motor_right_wheel_throttle_percent = config.motor_right_wheel_throttle_percent,
        motor_fan_throttle_percent = config.motor_fan_throttle_percent,
        motor_fan_idle_throttle_percent = config.motor_fan_idle_throttle_percent,
        arm_wait_ms = config.arm_wait_ms,
        on_duration_ms = config.on_duration_ms,
        on_delay_ms = config.on_delay_ms,
    )
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    task_id: usize,
    stack: Stack<'static>,
    config: &'static picoserve::Config,
    flash_storage: &'static storage::FlashMutex,
    rustwood_config: &'static RustwoodConfigMutex,
) {
    let app = Router::new().route(
        "/",
        routing::get(async move || {
            let config = {
                let c = rustwood_config.lock().await;
                *c
            };
            let html = render_page(&config, None);
            (("content-type", "text/html"), html)
        })
        .post(async move |Form(data): Form<FormData>| {
            let config = build_rustwood_config(&data);

            match data.action {
                FormAction::Apply => {
                    {
                        let mut c = rustwood_config.lock().await;
                        *c = config;
                    }

                    defmt::info!(
                        "web: apply throttle={}%,{}%,{}%,{}%,{}%, arm_wait={}ms, duration={}ms, delay={}ms",
                        config.motor_spare_throttle_percent,
                        config.motor_left_wheel_throttle_percent,
                        config.motor_right_wheel_throttle_percent,
                        config.motor_fan_throttle_percent,
                        config.motor_fan_idle_throttle_percent,
                        config.arm_wait_ms,
                        config.on_duration_ms,
                        config.on_delay_ms
                    );

                    let html = render_page(&config, Some("Applied live settings"));
                    (("content-type", "text/html"), html)
                }
                FormAction::Save => match storage::save_rustwood_config(flash_storage, &config).await {
                    Ok(()) => {
                        {
                            let mut c = rustwood_config.lock().await;
                            *c = config;
                        }

                        defmt::info!(
                            "web: save throttle={}%,{}%,{}%,{}%,{}%, arm_wait={}ms, duration={}ms, delay={}ms",
                            config.motor_spare_throttle_percent,
                            config.motor_left_wheel_throttle_percent,
                            config.motor_right_wheel_throttle_percent,
                            config.motor_fan_throttle_percent,
                            config.motor_fan_idle_throttle_percent,
                            config.arm_wait_ms,
                            config.on_duration_ms,
                            config.on_delay_ms
                        );

                        let html = render_page(&config, Some("Saved settings to flash"));
                        (("content-type", "text/html"), html)
                    }
                    Err(err) => {
                        let html = render_page(&config, Some(&format!("Failed to save config: {:?}", err)));
                        (("content-type", "text/html"), html)
                    }
                },
            }
        }),
    );

    let mut tcp_rx = [0u8; 1024];
    let mut tcp_tx = [0u8; 1024];
    let mut http_buf = [0u8; 2048];

    picoserve::Server::new(&app, config, &mut http_buf)
        .listen_and_serve(task_id, stack, 80, &mut tcp_rx, &mut tcp_tx)
        .await;
}
