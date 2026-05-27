# RGB LED Plan

Replace the GPIO6 digital LED with the built-in NeoPixel on GPIO48, while keeping the LED API general-purpose. The implementation should expose one helper that accepts full RGB plus brightness, then layer shortcut constructors or constants for blue, red, and green on top so the task logic stays readable while still supporting arbitrary colors later.

## Steps

1. Confirm the NeoPixel driver pattern that fits this codebase and wire it through the ESP32-S3 RMT path in `Cargo.toml` and `src/bin/main.rs`.
2. In `src/lib.rs`, add a small LED color abstraction centered on a full-value constructor such as `rgb_with_brightness(r, g, b, brightness)` or an equivalent `RgbColor` type with a brightness-scaling method.
3. In that same helper surface, add named shortcuts for the current state colors so task code can use `blue(...)`, `green(...)`, and `red(...)` rather than repeating raw channel values.
4. In `src/bin/main.rs`, remove the GPIO6 `Output` setup, initialize the built-in NeoPixel on GPIO48, and set the startup color to blue using the new helper API.
5. Update `switch_monitor_task` in `src/bin/main.rs` to depend on the NeoPixel writer instead of the old digital LED, and keep the state model explicit: blue while idle, red while the switch is held closed and armed, and green during the active timeout window.
6. Update `README.md` to replace the GPIO6 LED description with the built-in NeoPixel, and document the three runtime color states plus the fact that the helper supports arbitrary RGB with brightness.

## Relevant Files

- `Cargo.toml` — add or adjust NeoPixel-related dependency support
- `src/lib.rs` — add the reusable RGB plus brightness helper and named color shortcuts
- `src/bin/main.rs` — replace GPIO6 output handling with NeoPixel initialization and state updates
- `README.md` — update hardware and behavior documentation

## Verification

1. Run `source ~/export-esp.sh && cargo build` and confirm the firmware compiles with the selected NeoPixel dependency and helper API.
2. Flash to the ESP32-S3 board and confirm startup shows blue.
3. Close the GPIO4 switch and confirm the NeoPixel shows red for the armed/closed state.
4. Release the switch and confirm the NeoPixel turns green for the configured timeout duration while the PWM LED on GPIO5 remains active.
5. After the timeout expires, confirm the NeoPixel returns to blue when no longer active and not closed.
6. Exercise at least one non-shortcut RGB plus brightness call site so the general helper is validated, not just the named shortcuts.

## Decisions

- The LED helper should be general-purpose first, with named colors implemented as wrappers.
- Brightness should be handled in one place via channel scaling, so both arbitrary RGB values and red, green, and blue shortcuts behave consistently.
- Red should mean the switch is closed and armed, not the post-timeout idle state.
- Scope includes the hardware LED path and documentation update; it does not include adding web controls for RGB selection unless requested later.
