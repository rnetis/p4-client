# p4-client

ESP32-P4 touch thin client ("P4 Client") for the Waveshare ESP32-P4-WiFi6-Touch-LCD-7B.
See `PRD.md` (repo root, one level up) for the full product requirements.

## Layout

```
p4-client/
├── crates/           # Platform-independent Rust libraries (host-testable)
│   ├── p4-core       # App trait, AppManager, HAL traits, errors, logging
│   ├── p4-settings   # Persistent settings (TOML) + VNC profiles + secret store
│   ├── p4-input      # InputEvent model, gesture recognizer, touch->mouse mapping
│   ├── p4-network    # Wi-Fi state machine + auto-reconnect policy
│   ├── p4-storage    # Storage abstraction, path sanitization, host VFS
│   ├── p4-vnc        # RFB protocol client: handshake, auth, decoders, framebuffer
│   ├── p4-reader     # TXT/Markdown/EPUB parsing, pagination, bookmarks
│   └── p4-ui         # Headless UI model: launcher, status bar, virtual keyboard, themes
├── apps/             # Applications composed from the crates (launcher, vnc, reader, files, settings)
├── firmware/         # ESP-IDF project skeleton for the ESP32-P4 (board/display/touch/usb/wifi/power HAL)
├── tests/            # Integration tests (incl. full VNC handshake against a mock RFB server)
├── docs/             # Architecture and firmware integration notes
├── scripts/          # Build/test helper scripts
└── .github/workflows # CI (fmt, clippy, test, build)
```

## Development rules (from PRD §33)

1. Hardware abstraction — apps never touch GPIO/LCD/ESP-IDF directly; they use `p4-core::hal` traits.
2. Offline first — local apps work with no network.
3. No blocking UI — network/filesystem/VNC decoding run off the UI path.
4. Explicit resource ownership — RAII over globals.
5. Recoverable failures — an app crash must not take the firmware down.
6. Small modules — many focused crates.
7. Hardware-first validation — ESP-IDF APIs must be validated on the real board (see `firmware/`).

## Commands

```
cargo fmt && cargo clippy --workspace && cargo test --workspace   # host CI
./scripts/test.sh                                                  # same, convenient
./scripts/build-firmware.sh                                        # requires ESP-IDF v5.x env
```

## Status

Host-testable application layer is implemented and unit/integration tested.
Firmware bring-up (Phase 0) targets the real board and requires the ESP-IDF
toolchain plus hardware; see `firmware/README.md`.
