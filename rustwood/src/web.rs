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
    duty_pct: u8,
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
            let (duty, delay) = {
                let c = led_config.lock().await;
                (c.duty_pct, c.on_delay_ms)
            };
            let html = format!(
                "<!DOCTYPE html><html><head>\
<meta charset=utf-8><title>rustwood</title></head><body>\
<h2>rustwood LED config</h2>\
<form method=POST action=/>\
<label>Duty % (0-100): <input type=number name=duty_pct min=0 max=100 value={duty}></label><br><br>\
<label>On-delay ms: <input type=number name=on_delay_ms min=0 value={delay}></label><br><br>\
<button type=submit>Apply</button>\
</form></body></html>"
            );
            (("content-type", "text/html"), html)
        })
        .post(async move |Form(data): Form<FormData>| {
            let duty = data.duty_pct.min(100);
            {
                let mut c = led_config.lock().await;
                c.duty_pct = duty;
                c.on_delay_ms = data.on_delay_ms;
            }
            defmt::info!("web: duty={}% delay={}ms", duty, data.on_delay_ms);
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
