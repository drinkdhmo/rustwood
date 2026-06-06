## Plan: Persist web config to flash

Add a small NVS-backed settings layer so `LedConfig` is loaded on boot and can be written back from the web UI without changing the current live-apply flow. The form should keep its existing Apply behavior, and the new Save button should submit the same form values, update the shared config mutex, and then persist those values to flash so they survive power cycles.

**Steps**
1. Add the persistence dependency and data model support in the smallest possible surface area, reusing `LedConfig` as the single source of truth for both live state and stored state.
2. Introduce a storage helper module that can load an optional saved config on startup and save the current config on demand using ESP32 NVS. This should be the only place that knows the NVS key names and versioning scheme.
3. Update startup initialization in `src/bin/main.rs` to load persisted config before creating the shared mutex, falling back to `LedConfig::default()` when nothing is saved or decode fails.
4. Extend the web handler in `src/web.rs` so the page has two actions: Apply updates RAM only, and Save applies the submitted form then persists it to flash. Keep the existing live-config read path unchanged.
5. Add minimal validation and failure handling for storage operations so a save failure returns a clear response and does not crash the firmware.
6. Validate with a targeted firmware build and, if possible, a host-side test or compile check for the new config encoding/decoding path.

**Relevant files**
- `/home/bidscc/git/rustwood/Cargo.toml` — add the NVS/persistence dependency and any serialization support needed for `LedConfig`.
- `/home/bidscc/git/rustwood/src/lib.rs` — keep `LedConfig` as the shared config type and add any helpers needed for storing/loading it.
- `/home/bidscc/git/rustwood/src/bin/main.rs` — load persisted config during boot before `LED_CONFIG_CELL` is initialized.
- `/home/bidscc/git/rustwood/src/web.rs` — add a second form action/button and route the save path separately from Apply.
- `/home/bidscc/git/rustwood/src/storage.rs` — new helper for NVS load/save logic, if the implementation is split out.

**Verification**
1. Build the firmware for the embedded target with the ESP toolchain enabled.
2. Confirm the web page renders both Apply and Save actions.
3. Apply a config, Save it, reboot, and verify the same values load from flash before the first button press.
4. Confirm that a failed save returns a non-panicking error response and leaves the live config unchanged.

**Decisions**
- Use ESP32 NVS rather than a custom flash layout because the project only needs to persist a handful of settings and NVS keeps the implementation compact.
- Save should operate on the submitted form values, but the implementation should still update the in-memory mutex first so the live config and stored config stay consistent.
- Persistence scope is limited to `LedConfig`; no other runtime state should be written to flash.
