# rustwood

`rustwood` is a `no_std` Rust firmware project for the ESP32-S3 using `esp-hal`, `esp-rtos`, `embassy`, and `defmt`.

The current application monitors a switch on `GPIO4`. When the switch is released, it briefly debounces the input, turns on two LEDs for 1.5 seconds, then turns them back off:

- `GPIO5`: PWM-driven LED via the LEDC peripheral
- `GPIO6`: digital on/off LED

The repository also includes a Wokwi simulation setup so the same firmware can be exercised without physical hardware.

## Project Layout

- `src/bin/main.rs`: application entry point and switch / LED behavior
- `src/lib.rs`: crate library root
- `build.rs`: linker configuration and helpful linker error hints
- `.cargo/config.toml`: target, runner, linker, and Rust flags
- `diagram.json`: Wokwi wiring diagram
- `wokwi.toml`: Wokwi firmware and ELF paths

## Hardware / Simulation Wiring

The included Wokwi diagram models the following connections:

- `GPIO4` -> slide switch
- `GPIO5` -> green LED anode
- `GPIO6` -> red LED anode
- all LED cathodes and the switch ground side -> `GND`
- UART monitor -> `GPIO43` / `GPIO44`

## Firmware Behavior

At startup the firmware:

- initializes the ESP32-S3 HAL
- configures `GPIO4` as an input with internal pull-up
- configures LEDC timer 0 and channel 0 for PWM output on `GPIO5`
- configures `GPIO6` as a digital output
- emits `defmt` and serial log messages

During runtime it:

- waits for the switch to go low and then high again
- waits 20 ms for debounce
- if the switch is still high, turns both LEDs on
- keeps them on for 1.5 seconds
- turns both LEDs off and resumes waiting

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

## Build

Build the firmware with:

```sh
cargo build
```

The output configured for Wokwi is:

```text
target/xtensa-esp32s3-none-elf/debug/rustwood
```

## Run On Hardware

Because `.cargo/config.toml` sets a runner for the ESP32-S3, you can use:

```sh
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