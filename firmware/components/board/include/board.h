/*
 * Board pin map and panel timing for the
 * Waveshare ESP32-P4-WiFi6-Touch-LCD-7B.
 *
 * Panel: 7.0" 1024x600 MIPI-DSI, EK79007 driver IC, 2 data lanes,
 * same panel family as the ESP32-P4-Function-EV-Board MIPI display
 * (esp_lcd_ek79007, DPI clock 52 MHz).
 *
 * VERIFY ON HARDWARE (PRD Rule 7): the *_GPIO values below follow the
 * Waveshare 7B wiki schematic; if the panel stays dark or the log reports
 * a reset/power error, re-check BL_EN / LCD_RST / power-enable pins
 * against the board revision.
 */
#pragma once

#include "driver/gpio.h"

/* ---------------- Panel geometry / timing (EK79007, 1024x600) ----------- */
#define BOARD_PANEL_WIDTH_PX   1024
#define BOARD_PANEL_HEIGHT_PX  600
#define BOARD_PANEL_DPI_HZ     (52 * 1000 * 1000)   /* 52 MHz pixel clock */
#define BOARD_PANEL_LANES      2                     /* MIPI DSI data lanes */
#define BOARD_PANEL_COLOR_DEPTH 24                    /* RGB888 in, DSC out */

/* ---------------- MIPI-DSI host pins (fixed by silicon) ----------------- */
#define BOARD_PIN_DSI_CLK  GPIO_NUM_26
#define BOARD_PIN_DSI_D0   GPIO_NUM_24
#define BOARD_PIN_DSI_D1   GPIO_NUM_25

/* ---------------- Panel control ----------------------------------------- */
#define BOARD_PIN_LCD_RST  GPIO_NUM_27   /* panel reset, active low  (VERIFY) */
#define BOARD_PIN_LCD_BL   GPIO_NUM_23   /* backlight enable, high = on (VERIFY) */

/* ---------------- Boot feedback ------------------------------------------ */
#define BOARD_PIN_STATUS_LED GPIO_NUM_NC  /* set when the board has a usable LED */
