use embassy_net::{Config, Ipv4Address, Ipv4Cidr, Runner, Stack, StackResources, StaticConfigV4};
use embassy_executor::Spawner;
use esp_radio::wifi::{ControllerConfig, Interface, WifiController, ap::AccessPointConfig};

use crate::mk_static;

/// Drives the embassy-net stack. Must be spawned and kept running.
#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await
}

/// Starts the WiFi access point "rustwood" (open, no password) with static IP
/// 192.168.4.1/24 and returns the controller and network stack.
///
/// The returned `WifiController` **must** be kept alive for the lifetime of the
/// application — dropping it deinitialises the WiFi driver.
pub async fn start_ap(
    wifi: esp_hal::peripherals::WIFI<'static>,
    spawner: &Spawner,
) -> (WifiController<'static>, Stack<'static>) {
    let controller_config = ControllerConfig::default().with_initial_config(
        esp_radio::wifi::Config::AccessPoint(
            AccessPointConfig::default().with_ssid("rustwood"),
        ),
    );

    let (controller, interfaces) =
        esp_radio::wifi::new(wifi, controller_config).expect("wifi init failed");

    let net_config = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
        gateway: Some(Ipv4Address::new(192, 168, 4, 1)),
        dns_servers: Default::default(),
    });

    // A fixed seed is fine for an isolated AP — uniqueness is not required here.
    let seed: u64 = 0x926d_3c3c_b452_4462;

    let (stack, runner) = embassy_net::new(
        interfaces.access_point,
        net_config,
        mk_static!(StackResources<4>, StackResources::new()),
        seed,
    );

    spawner.spawn(net_task(runner).expect("net_task spawn failed"));

    // With a static IP the config is applied immediately; this resolves quickly.
    stack.wait_config_up().await;

    defmt::info!("AP ready — 192.168.4.1");
    (controller, stack)
}
