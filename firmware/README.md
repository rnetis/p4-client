# ESP-IDF firmware (PRD §5.1, Phase 0)

ESP-IDF v5.2 project for the Waveshare ESP32-P4-WiFi6-Touch-LCD-7B.

## Responsibilities
Hardware init, FreeRTOS, Wi-Fi (via ESP32-C6 / ESP-Hosted), USB host HID,
LCD (MIPI-DSI), touch (GT911-class I2C), SD/FAT storage, power management,
OTA (dual partitions + rollback, see `partitions.csv`), watchdog.

## Structure (PRD §5.2)
```
firmware/
├── main/            # entry point: boot sequence + Rust bridge (FFI)
├── components/
│   ├── board/       # board-specific pin/clock config (7B)
│   ├── display/     # LCD init + framebuffer flush
│   ├── touch/       # touch controller driver -> InputEvent touch stream
│   ├── usb/         # USB OTG host: HID keyboard/mouse
│   ├── wifi/        # ESP-Hosted C6 Wi-Fi
│   └── power/       # battery gauge, sleep, dimming
```

## Build
```
. $IDF_PATH/export.sh
idf.py set-target esp32p4
idf.py build
idf.py -p /dev/ttyACM0 flash monitor
```
`sdkconfig.defaults` (repo root) is applied automatically on first configure.

## Rust integration
The application layer (crates/ + apps/) is pure, host-testable Rust. It is
linked into the firmware via `esp-idf-sys`/`esp-idf-svc` bindings: the HAL
traits in `p4-core::hal` get thin ESP-IDF implementations here
(`Display`, `TouchDevice`, `Clock`, `KeyValueStore`, `KeyProvider` from
eFuse). PRD Rule 7: every peripheral call must be validated on the real
board before being relied upon.
