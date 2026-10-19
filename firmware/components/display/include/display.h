/* Display HAL for the EK79007 MIPI-DSI panel (see board.h for geometry). */
#pragma once

#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Bring up the DSI bus, panel, framebuffer, and backlight.
 *  Leaves a solid dark-blue test color on screen. */
esp_err_t display_init(void);

/** Fill the whole framebuffer with one RGB color (next flush shows it). */
void display_fill(uint8_t r, uint8_t g, uint8_t b);

/** Flush the classic bring-up pattern: 8 color bars (top) + grayscale
 *  gradient (bottom). Exposes lane/color-order/porching errors. */
void display_show_test_pattern(void);

#ifdef __cplusplus
}
#endif
