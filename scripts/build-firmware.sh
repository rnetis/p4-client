#!/usr/bin/env bash
# Build the ESP-IDF firmware for the ESP32-P4 board (PRD Phase 0).
# Requires the ESP-IDF v5.x environment (`. $IDF_PATH/export.sh`).
set -euo pipefail
cd "$(dirname "$0")/../firmware"
idf.py set-target esp32p4
idf.py build
idf.py -p PORT flash monitor   # PORT: e.g. /dev/ttyACM0
