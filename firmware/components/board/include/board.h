/*
 * Board pin map and panel config for the
 * Waveshare ESP32-P4-WiFi6-Touch-LCD-7B.
 *
 * Values taken from Waveshare's own esp-idf example
 * (ESP32-P4-WIFI6-Touch-LCD-7B/examples/esp-idf/07_color_panel):
 *   - panel: 1024x600 EK79007, MIPI-DSI, 2 data lanes
 *   - LCD reset: GPIO 33 (active low, driven by the panel driver)
 *   - backlight: GPIO 32, ACTIVE LOW (low = lit)
 *   - the MIPI-DSI PHY is powered from the on-chip LDO channel 3 @ 2500 mV;
 *     without this the PHY stays in "No Power" state and the panel is black
 *     (this was the v0.2.0 black-screen root cause).
 */
#pragma once

#include "driver/gpio.h"

/* ---------------- Panel geometry / timing (EK79007, 1024x600) ----------- */
#define BOARD_PANEL_WIDTH_PX   1024
#define BOARD_PANEL_HEIGHT_PX  600
#define BOARD_PANEL_LANES      2      /* MIPI DSI data lanes */
#define BOARD_PANEL_BPP        24     /* RGB888 */

/* ---------------- MIPI-DSI PHY power (on-chip LDO) ---------------------- */
#define BOARD_DSI_PHY_LDO_CHAN        3
#define BOARD_DSI_PHY_LDO_VOLTAGE_MV  2500

/* ---------------- Panel control ----------------------------------------- */
#define BOARD_PIN_LCD_RST     GPIO_NUM_33  /* active low; driver-managed */
#define BOARD_PIN_LCD_BL      GPIO_NUM_32  /* ACTIVE LOW: low = lit      */
#define BOARD_LCD_BL_ON_LEVEL 0
