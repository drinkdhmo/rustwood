# Plan: WiFi AP + Web Config UI for rustwood

## Summary
Add a WiFi Access Point to the ESP32-S3 so a browser on the same network can configure the LED duty cycle and on-delay. Uses `esp-radio` (AP mode), `embassy-net` (static IP), and `picoserve` (HTTP). Shared state between web and switch tasks via `embassy_sync::Mutex`.

## Key Technical Decisions
- AP SSID: "rustwood", open (no password)
- Static IP: 192.168.4.1/24 — no DHCP server; user manually sets client IP to 192.168.4.x
- Shared config: `Mutex<CriticalSectionRawMutex, RustwoodConfig>` in a `StaticCell`, shared by ref
- Form POST handling: `picoserve::extract::Form<FormData>` with `serde::Deserialize`
- `WifiController` kept alive in `main()` (never dropped)
- Pool size 2 web tasks, `StackResources<4>`

## Files

### Modified
- `Cargo.toml` — add `serde` (no_std, derive), `embassy-sync` explicitly
- `src/lib.rs` — add `#![feature(impl_trait_in_assoc_type)]`, `pub mod web; pub mod wifi;`, `mk_static!` macro, `RustwoodConfig` struct
- `src/bin/main.rs` — add heap allocator, WiFi AP init, embassy-net stack, spawn net_task + web tasks; pass `&LED_CONFIG` to switch task; update switch task signature

### New
- `src/wifi.rs` — `net_task`, `start_ap()` fn returning `(WifiController, Stack<'static>)`
- `src/web.rs` — `AppState`, `Application: AppWithStateBuilder`, GET `/` + POST `/`, `web_task`

## Steps

### Phase 1: Dependencies & Lib (parallel)
1. **`Cargo.toml`**: Add `serde = { version = "1", default-features = false, features = ["derive"] }` and `embassy-sync = { version = "0.8" }`
2. **`src/lib.rs`**: Add `#![feature(impl_trait_in_assoc_type)]`, `pub mod web; pub mod wifi;`, `mk_static!` macro, `RustwoodConfig { duty_pct: u8, on_duration_ms: u64 }` struct with `Default` (75, 1500)

### Phase 2: WiFi AP Module
3. **`src/wifi.rs`** (new):
   - `#[embassy_executor::task] async fn net_task(runner: Runner<'static, Interface<'static>>) { runner.run().await }`
   - `pub async fn start_ap(wifi: WIFI<'static>, spawner: &Spawner) -> (WifiController<'static>, Stack<'static>)`:
     - `ControllerConfig::default().with_initial_config(Config::AccessPoint(AccessPointConfig::default().with_ssid("rustwood")))`
     - `let (controller, interfaces) = esp_radio::wifi::new(wifi, config)?`
     - Static IP: `embassy_net::Config::ipv4_static(StaticConfigV4 { address: Ipv4Cidr::new(Ipv4Address::new(192,168,4,1), 24), gateway: Some(192.168.4.1), dns_servers: Default::default() })`
     - `embassy_net::new(interfaces.access_point, net_config, mk_static!(StackResources<4>, ...), seed)`
     - Spawn `net_task(runner)`; wait loop on `stack.is_link_up()`
     - Return `(controller, stack)` — caller keeps controller alive in scope

### Phase 3: Web Module
4. **`src/web.rs`** (new):
   - `pub struct AppState { pub rustwood_config: &'static Mutex<CriticalSectionRawMutex, RustwoodConfig> }`
   - `#[derive(serde::Deserialize)] struct FormData { duty_pct: u8, on_duration_ms: u64 }`
   - `pub struct Application(pub AppState)` implementing `AppWithStateBuilder`:
     - **GET `/`**: lock mutex, format HTML form with current values, return HTML response
     - **POST `/`**: extract `Form<FormData>`, validate `duty_pct` 0–100, lock mutex, update values, return confirmation HTML
   - `pub const WEB_TASK_POOL_SIZE: usize = 2;`
   - `#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)] pub async fn web_task(...)` — port 80, 1024-byte TCP buffers, 2048-byte HTTP buffer

### Phase 4: Wire main.rs
5. **`src/bin/main.rs`**:
   - Add `esp_alloc::heap_allocator!(size: 72 * 1024);` after `esp_hal::init`
   - `static LED_CONFIG: StaticCell<Mutex<CriticalSectionRawMutex, RustwoodConfig>> = StaticCell::new();` — init with `RustwoodConfig::default()`
   - `let (_controller, stack) = lib::wifi::start_ap(peripherals.WIFI, &spawner).await;`
   - Build `AppState`, build `Application(AppState { rustwood_config }).build_app()`
   - Spawn 2 web tasks
   - Pass `rustwood_config` ref to `switch_monitor_task`

### Phase 5: switch_monitor_task Update
6. **`switch_monitor_task`** in `src/bin/main.rs`:
   - Add `config: &'static Mutex<CriticalSectionRawMutex, RustwoodConfig>` parameter
   - Before LED activation: `let (duty, delay_ms) = { let c = config.lock().await; (c.duty_pct, c.on_duration_ms) };`
   - Replace hardcoded `75` → `duty` and `1500` → `delay_ms`

## Verification
1. `source ~/export-esp.sh && cargo build` — zero errors
2. Flash; connect to SSID **rustwood** (open)
3. Manually assign client IP `192.168.4.2/24`, gateway `192.168.4.1`
4. Browse to `http://192.168.4.1/` — form shows current duty/delay
5. Submit new values; trigger GPIO4 switch — LEDs use updated settings
6. Confirm via defmt log that mutex values are being read

## Decisions / Scope
- **No DHCP server**: `embassy-net` 0.9 has no AP-side DHCP server; clients must set a static IP. Can be improved later.
- **Open AP**: no WPA2 password for development simplicity — add `with_password(...)` to `AccessPointConfig` later.
- **No NVS persistence**: config resets to defaults (75%, 1500 ms) on reboot.
- **serde for Form**: `picoserve::extract::Form<T: DeserializeOwned>` is idiomatic; fallback is manual `url_encoded` parsing if needed.
