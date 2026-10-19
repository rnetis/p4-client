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

## Prerequisites

- **Host tests:** any stable Rust toolchain (1.75+ recommended).
- **Firmware build/flash:** [ESP-IDF v5.4+](https://docs.espressif.com/projects/esp-idf/en/v5.4.1/esp32p4/get-started/) — required, not optional: the 7B ships ESP32-P4 silicon rev v1.3, which IDF v5.3 refuses to flash (chip-revision limit v0.99), and v5.2 has no MIPI-DSI driver at all. Plus:
  - a USB-C data cable (the board exposes a USB-Serial-JTAG port)
  - a Linux/macOS host with `/dev/ttyACM*` access (add yourself to `dialout`/`uucp` if permission is denied)

## Host build & test (no hardware needed)

```
cargo build --workspace
cargo test  --workspace     # 84 tests, all host-testable
cargo fmt && cargo clippy --workspace
# or the convenient wrapper:
./scripts/test.sh
```

## Full firmware build

1. Install ESP-IDF v5.4+ and set up its environment:

   ```
   git clone -b v5.4.1 --recursive https://github.com/espressif/esp-idf.git ~/esp-idf
   cd ~/esp-idf && ./install.sh esp32p4
   . ./export.sh                      # do this in every new shell
   ```

2. Build the firmware (from this repo):

   ```
   . $IDF_PATH/export.sh
   cd firmware
   idf.py set-target esp32p4
   idf.py build                       # applies repo-root sdkconfig.defaults
   ```

   Or simply: `./scripts/build-firmware.sh` (with the IDF env sourced).

   The build applies `sdkconfig.defaults` automatically on first configure:
   360 MHz CPU, 16 MB flash (QIO @ 200 MHz), PSRAM @ 200 MHz, 1 kHz FreeRTOS
   tick, ESP-Hosted (Wi-Fi via the on-board ESP32-C6), USB-Serial-JTAG console,
   custom partition table (see `firmware/partitions.csv`: NVS, PHY, otadata,
   two 6.25 MB OTA slots for dual-image OTA with rollback, FAT storage).

## Writing to the device (flash & monitor)

1. Connect the board by USB-C and identify the port (usually `/dev/ttyACM0`
   on Linux; check `ls /dev/ttyACM*` or `dmesg` right after plugging in).

2. Flash and open the serial monitor:

   ```
   . $IDF_PATH/export.sh
   cd firmware
   idf.py -p /dev/ttyACM0 flash monitor
   ```

   Or with the helper script (PORT is your serial device):

   ```
   ./scripts/build-firmware.sh        # builds, then edit the PORT in it
   ```

3. Expected first-boot output: the `p4-boot` log prints each boot stage
   (hardware init → display → storage → Wi-Fi → config → UI, see
   `firmware/main/main.c`). Since v0.2.3 the MIPI-DSI panel is initialized
   (`firmware/components/display/`, EK79007 driver) and the screen shows a
   **test pattern** (solid blue → DSI hardware bars → color bars + grayscale
   gradient) with backlight on. **Display bring-up is validated on real
   hardware.** If a future build shows a black screen, check the serial log
   for `DISPLAY INIT FAILED` and the panel control pins in
   `firmware/components/board/include/board.h`.
   Storage/Wi-Fi/UI bring-up remains scaffolded for later Phase-0 work.

4. Exit the monitor with `Ctrl-]`.

### Erasing flash (clean re-flash)

```
idf.py -p /dev/ttyACM0 erase-flash flash monitor
```

> PRD Rule 7: every peripheral call must be validated on the real board
> before being relied upon — treat early flashes as bring-up experiments.

## Status

Host-testable application layer is implemented and unit/integration tested
(84/84 green). Firmware: **MIPI-DSI display bring-up validated on real
hardware** (EK79007 panel, test pattern on boot, chip rev v1.3 needs
IDF v5.4+). Storage/Wi-Fi/UI bring-up still scaffolded — see
`firmware/README.md`.
