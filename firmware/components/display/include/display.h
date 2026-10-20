/* Display HAL for the EK79007 MIPI-DSI panel (see board.h for geometry). */
#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "esp_lcd_panel_ops.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Bring up the DSI bus, panel, framebuffer, and backlight.
 *  Leaves a solid dark-blue test color on screen. */
esp_err_t display_init(void);

/** Handle of the initialized DPI panel (for esp_lvgl_port etc.). */
esp_lcd_panel_handle_t p4_panel_handle(void);

/** Fill the whole framebuffer with one RGB color (next flush shows it). */
void display_fill(uint8_t r, uint8_t g, uint8_t b);

/** Flush the classic bring-up pattern: 8 color bars (top) + grayscale
 *  gradient (bottom). Exposes lane/color-order/porching errors. */
void display_show_test_pattern(void);

/** DSI-hardware-generated pattern (no framebuffer, no panel commands).
 *  Diagnostic: on = vertical bars. Returns ESP_ERR_NOT_SUPPORTED if the
 *  driver has no pattern capability. */
esp_err_t display_hw_pattern(bool on);

#ifdef __cplusplus
}
#endif
