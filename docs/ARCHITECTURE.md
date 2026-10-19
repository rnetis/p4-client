# Architecture (PRD §4, §36)

```
UI (LVGL, on-device)  ──  headless model: p4-ui
        │
Applications: launcher / vnc / reader / files / settings  (apps/*)
        │  App trait + AppManager (p4-core)
Runtime crates:
  p4-settings  persistent TOML settings, VNC profiles, AES-GCM secret store
  p4-input     InputEvent model, gestures, touch->mouse
  p4-network   Wi-Fi state machine + auto-reconnect
  p4-storage   volume abstraction, path sanitization
  p4-vnc       RFB 3.8 client, DES auth, Raw/CopyRect decoders, RGB565 FB
  p4-reader    TXT/MD/EPUB parse, pagination, bookmarks, resume
        │
HAL traits (p4-core::hal): Clock, Display, TouchDevice, Battery,
KeyValueStore, KeyProvider  ── implemented in firmware/ (ESP-IDF)
```

## Key decisions

- **App layer is pure Rust and host-tested.** All logic that does not need
  the radio/screen lives in crates/ and runs under `cargo test` (PRD Rule 1, 6).
- **VNC decodes straight into the RGB565 framebuffer** with dirty-rect
  tracking; no intermediate copies (PRD §10, §28).
- **Passwords are AES-256-GCM encrypted** with a platform key
  (`KeyProvider`; eFuse on device) — never persisted as plaintext (PRD §26).
- **Path sanitization is centralized** in `p4_storage::sanitize`; traversal
  attempts are rejected as security errors.
- **Deletion requires explicit confirmation** at the app layer (PRD §15).
- **Firmware skeleton** (firmware/) holds the ESP-IDF boot sequence and the
  future HAL implementations; bring-up must be validated on the real board
  (PRD Rule 7), which requires hardware.
