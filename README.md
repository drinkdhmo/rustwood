# rustwood

`rustwood` is a `no_std` Rust firmware project for the ESP32-S3 using `esp-hal`, `esp-rtos`, `embassy`, and `defmt`.

The application monitors a switch on `GPIO4`. When the switch is pressed and held for a configurable arming delay, then released, it drives four PWM motor outputs at configurable throttle percentages (0-100%) after an optional activation delay. The NeoPixel LED provides feedback through multiple state colors:

- `GPIO5`-`GPIO8`: motor PWM outputs via the LEDC peripheral (50 Hz, duty cycle derived from throttle percentage)
- `GPIO48`: built-in NeoPixel (WS2812-style) via RMT, driven through an RGB+brightness helper API

A WiFi access point named **rustwood** is broadcast on startup. Connecting a browser to `http://192.168.4.1` opens a configuration page where motor throttle percentages, arm delay, and timing parameters can be changed live without reflashing. Updated values take effect on the next button press.

The repository also includes a Wokwi simulation setup so the same firmware can be exercised without physical hardware.

## System Setup

This project uses rust. Ensure you have the prerequisites:

```sh
sudo apt update && sudo apt install curl build-essential -y
```

Then install rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Add the source to your rc file:
```sh
echo '. "$HOME/.cargo/env"' >> ~/.zshrc && source ~/.zshrc
```

Now install the toolchain (from https://esp32.implrust.com/dev-env.html), but make sure you are not in this folder when you run it or cargo will try to use the ESP toolchain that is not yet installed. Also, specific versions are included that are known to work with this project.
```sh
cargo install cargo-binstall
cargo binstall espflash@4.2.0
```
Verify the installation:
```sh
espflash --version
```
Now for `esp-generate`:
```sh
cargo install esp-generate@1.0.0 --locked
```
Next `espup`:
```sh
cargo binstall espup@0.16.0
espup install --toolchain-version 1.95.0
```
Finally, install `probe-rs`:
```sh
cargo install probe-rs-tools --locked
```

### Udev Rules

```sh
curl -sL https://probe.rs/files/69-probe-rs.rules | sudo tee /etc/udev/rules.d/69-probe-rs.rules > /dev/null
sudo udevadm control --reload
sudo udevadm trigger
```


## Project Layout

- `src/bin/main.rs`: application entry point — hardware init, WiFi start, task spawning
- `src/lib.rs`: shared types (`LedConfig`, `LedConfigMutex`) and the `mk_static!` macro
- `src/wifi.rs`: WiFi AP setup using `esp-radio` and `embassy-net`
- `src/web.rs`: picoserve HTTP server — GET renders the config form, POST updates it
- `build.rs`: linker configuration and helpful linker error hints
- `.cargo/config.toml`: target, runner, linker, and Rust flags
- `diagram.json`: Wokwi wiring diagram
- `wokwi.toml`: Wokwi firmware and ELF paths

## Hardware / Simulation Wiring

The included Wokwi diagram models the following connections:

- `GPIO4` -> push switch
- `GPIO5` -> spare motor PWM signal input
- `GPIO6` -> left wheel motor PWM signal input
- `GPIO7` -> right wheel motor PWM signal input
- `GPIO8` -> fan motor PWM signal input
- motor grounds and switch ground side -> `GND`
- UART monitor -> `GPIO43` / `GPIO44`

## Firmware Behavior

At startup the firmware:

- initializes the ESP32-S3 HAL and a 72 KB heap
- configures `GPIO4` as an input with internal pull-up
- configures LEDC timer 1 and channels 0-3 for 50 Hz motor PWM output on `GPIO5`-`GPIO8`
- configures RMT channel 0 for the built-in NeoPixel on `GPIO48`
- starts the WiFi AP (`rustwood`, 192.168.4.1/24)
- spawns two HTTP server tasks on port 80
- emits `defmt` and serial log messages

During runtime it implements a state machine with color feedback:

- **Idle (blue)**: waits for button press; fan runs at idle throttle
- **Arming (orange)**: switch held for `arm_wait_ms` (default 500 ms); NeoPixel shows arming in progress; if released during this window, sequence cancels and returns to idle
- **Armed (red)**: switch held through arm delay; the fan motor is run at the idle throttle
- **Switch released**: button is released after arming completes
- **Activation delay (yellow)**: waits for `on_delay_ms` before motors reach full throttle (default 0 ms)
- **Active run (green)**: all motors run at configured throttle percentages for `on_duration_ms` (default 1500 ms)
- **Idle (blue)**: motors return to zero throttle, resumes waiting

## Web Configuration UI

Connect a device to the **rustwood** WiFi network (no password) and open:

```
http://192.168.4.1
```

The page shows the current motor throttle percentages (0-100%) and timing parameters and lets you submit new values via a form POST. Changes are applied immediately to the shared config mutex and take effect on the next button press.

Configuration parameters:
- **Spare motor throttle**: 0-100% (default 0%)
- **Left wheel motor throttle**: 0-100% (default 20%)
- **Right wheel motor throttle**: 0-100% (default 20%)
- **Fan motor throttle**: 0-100% (default 20%)
- **Fan idle throttle**: 0-100% (default 0%, runs while idle)
- **Arm delay**: milliseconds to hold switch before arming (default 500 ms)
- **Activation delay**: milliseconds to wait after button release before reaching full throttle (default 0 ms)
- **Run duration**: milliseconds to run motors at full throttle (default 1500 ms)
## Requirements

You need an ESP Rust toolchain and linker capable of building for `xtensa-esp32s3-none-elf`.

This repository is configured for:

- Rust toolchain channel: `esp`
- target: `xtensa-esp32s3-none-elf`
- linker: `xtensa-esp32s3-elf-gcc`
- default runner: `probe-rs run --chip=esp32s3`

Typical tools you may need installed locally:

- ESP Rust toolchain with the `esp` channel
- `xtensa-esp32s3-elf-gcc`
- `probe-rs` for flashing / running on hardware
- Wokwi tooling if you want to run the simulator from the command line

## Environment Setup

Before building or running the firmware in a new shell, load the ESP toolchain paths:

```sh
source ~/export-esp.sh
```

That script exports the toolchain binaries needed by this project, including the Xtensa GCC linker.

## Build

Build the firmware with:

```sh
source ~/export-esp.sh
cargo build
```

The output configured for Wokwi is:

```text
target/xtensa-esp32s3-none-elf/debug/rustwood
```

## Testing

This workspace defaults to the embedded target (`xtensa-esp32s3-none-elf`), so plain `cargo test` will not work for unit tests that rely on Rust's standard test harness.

Run host-side library unit tests with:

```sh
cargo +stable test-host
```

The `test-host` alias is defined in `.cargo/config.toml` and runs:

```sh
cargo test --target x86_64-unknown-linux-gnu --lib
```

For firmware validation on the board target, continue using:

```sh
source ~/export-esp.sh
cargo build
```

## Run On Hardware

Because `.cargo/config.toml` sets a runner for the ESP32-S3, you can use:

```sh
source ~/export-esp.sh
cargo run
```

That will invoke `probe-rs run --chip=esp32s3` for the built artifact.

## Run In Wokwi

The repository already contains both:

- `diagram.json` for the board and wiring
- `wokwi.toml` for the ELF / firmware paths and GDB port

Build the project first so the firmware image exists, then launch the simulation with your preferred Wokwi workflow. Wokwi will use the generated firmware at:

```text
target/xtensa-esp32s3-none-elf/debug/rustwood
```

## Logging

This project uses both:

- `defmt` over RTT
- `esp-println` over UART

The default `DEFMT_LOG` level in `.cargo/config.toml` is `info`.

## Notes

- `build.rs` adds the linker scripts required for `defmt` and `linkall.x`.
- The project enables `build-std` for `core` and `alloc` to support the configured stack protection flags.
- The checked-in `target/` directory is build output and can be regenerated locally.