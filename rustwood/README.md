# rustwood

`rustwood` is a `no_std` Rust firmware project for the ESP32-S3 using `esp-hal`, `esp-rtos`, `embassy`, and `defmt`.

The application monitors a switch on `GPIO4`. When the switch is released it briefly debounces the input, then drives a PWM LED for a configurable duration while also updating the built-in NeoPixel state color:

- `GPIO5`: PWM-driven LED via the LEDC peripheral (brightness set by duty cycle %)
- `GPIO48`: built-in NeoPixel (WS2812-style) via RMT, driven through an RGB+brightness helper API

A WiFi access point named **rustwood** is broadcast on startup. Connecting a browser to `http://192.168.4.1` opens a configuration page where the duty cycle (0–100 %) and on-delay (ms) can be changed live without reflashing. Updated values take effect on the next button press.

The repository also includes a Wokwi simulation setup so the same firmware can be exercised without physical hardware.

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

- `GPIO4` -> slide switch
- `GPIO5` -> green LED anode
- all LED cathodes and the switch ground side -> `GND`
- UART monitor -> `GPIO43` / `GPIO44`

## Firmware Behavior

At startup the firmware:

- initializes the ESP32-S3 HAL and a 72 KB heap
- configures `GPIO4` as an input with internal pull-up
- configures LEDC timer 0 and channel 0 for PWM output on `GPIO5`
- configures RMT channel 0 for the built-in NeoPixel on `GPIO48`
- starts the WiFi AP (`rustwood`, 192.168.4.1/24)
- spawns two HTTP server tasks on port 80
- emits `defmt` and serial log messages

During runtime it:

- keeps the NeoPixel blue while idle
- waits for the switch to go low (closed) and then sets NeoPixel red (armed)
- waits for the switch to go high again
- waits 20 ms for debounce
- if the switch is still high, reads the current `duty_pct` and `on_delay_ms` from the shared mutex
- sets NeoPixel green during the active timeout window
- drives the PWM LED on `GPIO5` at the configured duty cycle for the configured delay
- returns NeoPixel to blue and resumes waiting

## Web Configuration UI

Connect a device to the **rustwood** WiFi network (no password) and open:

```
http://192.168.4.1
```

The page shows the current duty cycle and on-delay and lets you submit new values via a form POST. Changes are applied immediately to the shared config mutex and take effect on the next button press.

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