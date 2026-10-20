/*
 * display.c — MIPI-DSI panel bring-up for the EK79007 1024x600 panel.
 * Validated on real hardware (Waveshare ESP32-P4-WiFi6-Touch-LCD-7B).
 *
 * Init sequence (mirrors Waveshare's 07_color_panel example):
 *   1. LDO channel 3 @ 2500 mV powers the MIPI-DSI PHY (mandatory!)
 *   2. DSI host bus (2 lanes)
 *   3. DBI io -> DPI panel (vendor driver esp_lcd_ek79007, 60 Hz timing)
 *   4. esp_lcd_panel_reset + init (driver drives LCD_RST on GPIO33)
 *   5. framebuffer -> test pattern flush -> backlight on (GPIO32, low = lit)
 */
#include "display.h"

#include "board.h"
#include "driver/gpio.h"
#include "esp_ldo_regulator.h"
#include "esp_lcd_mipi_dsi.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_lcd_ek79007.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "p4-display";

/* One full-resolution RGB888 frame = 1024 * 600 * 3 = 1.8 MB (in PSRAM). */
#define FB_BYTES (BOARD_PANEL_WIDTH_PX * BOARD_PANEL_HEIGHT_PX * 3)

static esp_lcd_panel_handle_t s_panel;
static uint8_t *s_fb;
static esp_ldo_channel_handle_t s_ldo_phy;

esp_lcd_panel_handle_t p4_panel_handle(void)
{
    return s_panel;
}

esp_err_t display_init(void)
{
    esp_err_t err;

    /* 0. Power the MIPI-DSI PHY. The P4's DSI PHY boots in "No Power" state;
     *    without this LDO channel every later step silently does nothing. */
    esp_ldo_channel_config_t ldo_cfg = {
        .chan_id = BOARD_DSI_PHY_LDO_CHAN,
        .voltage_mv = BOARD_DSI_PHY_LDO_VOLTAGE_MV,
    };
    err = esp_ldo_acquire_channel(&ldo_cfg, &s_ldo_phy);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "DSI PHY LDO acquire failed: %s", esp_err_to_name(err));
        return err;
    }
    ESP_LOGI(TAG, "MIPI-DSI PHY powered (LDO chan %d, %d mV)",
             BOARD_DSI_PHY_LDO_CHAN, BOARD_DSI_PHY_LDO_VOLTAGE_MV);

    /* 1. MIPI-DSI host bus: 2 data lanes (vendor macro default 900 Mbps) */
    esp_lcd_dsi_bus_handle_t dsi_bus;
    esp_lcd_dsi_bus_config_t bus_cfg = EK79007_PANEL_BUS_DSI_2CH_CONFIG();
    err = esp_lcd_new_dsi_bus(&bus_cfg, &dsi_bus);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "DSI bus init failed: %s", esp_err_to_name(err));
        return err;
    }

    /* 2. DBI control interface (panel init commands go over the same bus) */
    esp_lcd_panel_io_handle_t io;
    esp_lcd_dbi_io_config_t dbi_cfg = EK79007_PANEL_IO_DBI_CONFIG();
    err = esp_lcd_new_panel_io_dbi(dsi_bus, &dbi_cfg, &io);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "DBI io init failed: %s", esp_err_to_name(err));
        return err;
    }

    /* 3. Panel via vendor driver: vendor config wires the DSI bus + DPI
     *    timing (1024x600 @ 60 Hz, 52 MHz pixel clock, RGB888). The driver
     *    drives LCD_RST (GPIO33) itself via esp_lcd_panel_reset(). */
    esp_lcd_dpi_panel_config_t dpi_cfg =
        EK79007_1024_600_PANEL_60HZ_CONFIG(LCD_COLOR_PIXEL_FORMAT_RGB888);
    ek79007_vendor_config_t vendor_cfg = {
        .mipi_config = {
            .dsi_bus = dsi_bus,
            .dpi_config = &dpi_cfg,
            .lane_num = BOARD_PANEL_LANES,
        },
    };
    esp_lcd_panel_dev_config_t dev_cfg = {
        .reset_gpio_num = BOARD_PIN_LCD_RST,
        .rgb_ele_order = LCD_RGB_ELEMENT_ORDER_RGB,
        .bits_per_pixel = BOARD_PANEL_BPP,
        .vendor_config = &vendor_cfg,
    };
    err = esp_lcd_new_panel_ek79007(io, &dev_cfg, &s_panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "EK79007 panel create failed: %s", esp_err_to_name(err));
        return err;
    }
    err = esp_lcd_panel_reset(s_panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "panel reset failed: %s", esp_err_to_name(err));
        return err;
    }
    err = esp_lcd_panel_init(s_panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "panel init (vendor cmd sequence) failed: %s", esp_err_to_name(err));
        return err;
    }

    /* 5. Framebuffer owned by the DPI panel driver (PSRAM) */
    void *fb_ptr = NULL;
    err = esp_lcd_dpi_panel_get_frame_buffer(s_panel, 1, &fb_ptr);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "get framebuffer failed: %s", esp_err_to_name(err));
        return err;
    }
    s_fb = fb_ptr;
    ESP_LOGI(TAG, "framebuffer at %p (%u KB)", s_fb, (unsigned)(FB_BYTES / 1024));

    /* The EK79007 driver may not implement disp_on_off (NOT_SUPPORTED) —
     * log it but never abort here: the DPI engine scans out regardless. */
    err = esp_lcd_panel_disp_on_off(s_panel, true);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "disp_on_off: %s (continuing)", esp_err_to_name(err));
    }

    /* 6. Prove pixels: solid dark blue first (any garbage is obvious) */
    display_fill(0x00, 0x00, 0x80);
    err = esp_lcd_panel_draw_bitmap(s_panel, 0, 0, BOARD_PANEL_WIDTH_PX, BOARD_PANEL_HEIGHT_PX, s_fb);
    if (err != ESP_OK) return err;

    /* 7. Backlight on last, so the user never sees an uninitialized panel.
     *    GPIO32 is ACTIVE LOW on this board: driving it low lights it up. */
    gpio_config_t bl_cfg = {
        .pin_bit_mask = 1ULL << BOARD_PIN_LCD_BL,
        .mode = GPIO_MODE_OUTPUT,
    };
    gpio_config(&bl_cfg);
    gpio_set_level(BOARD_PIN_LCD_BL, BOARD_LCD_BL_ON_LEVEL);
    ESP_LOGI(TAG, "backlight on (GPIO%d, active low)", BOARD_PIN_LCD_BL);

    ESP_LOGI(TAG, "display online: %dx%d, test pattern shown", BOARD_PANEL_WIDTH_PX, BOARD_PANEL_HEIGHT_PX);
    return ESP_OK;
}

/* DSI-controller-generated vertical color bars. Bypasses the framebuffer
 * and the panel's init commands entirely: if THIS is not visible, the
 * problem is below the driver (PHY power, lanes, DPI clock); if it IS
 * visible but the framebuffer pattern is not, the problem is panel init
 * commands or the framebuffer path. */
esp_err_t display_hw_pattern(bool on)
{
    if (!s_panel) return ESP_ERR_INVALID_STATE;
    return esp_lcd_dpi_panel_set_pattern(
        s_panel, on ? MIPI_DSI_PATTERN_BAR_VERTICAL : MIPI_DSI_PATTERN_NONE);
}

void display_fill(uint8_t r, uint8_t g, uint8_t b)
{
    if (!s_fb) return;
    for (uint32_t i = 0; i < FB_BYTES; i += 3) {
        s_fb[i + 0] = r;
        s_fb[i + 1] = g;
        s_fb[i + 2] = b;
    }
}

/* Horizontal color bars in the top half + grayscale gradient in the bottom
 * half: the classic bring-up pattern, exposes lane/color-order/porch issues. */
void display_show_test_pattern(void)
{
    if (!s_fb) return;
    const uint8_t bars[8][3] = {
        {255, 255, 255}, {255, 255, 0}, {0, 255, 255}, {0, 255, 0},
        {255, 0, 255}, {255, 0, 0}, {0, 0, 255}, {0, 0, 0},
    };
    uint32_t half = (uint32_t)BOARD_PANEL_HEIGHT_PX / 2 * BOARD_PANEL_WIDTH_PX * 3;
    for (uint32_t px = 0; px < half; px += 3) {
        uint32_t x = (px / 3) % BOARD_PANEL_WIDTH_PX;
        const uint8_t *c = bars[(x * 8) / BOARD_PANEL_WIDTH_PX];
        s_fb[px + 0] = c[0];
        s_fb[px + 1] = c[1];
        s_fb[px + 2] = c[2];
    }
    for (uint32_t px = half; px < FB_BYTES; px += 3) {
        uint8_t v = (uint8_t)(((px - half) * 255) / (FB_BYTES - half));
        s_fb[px + 0] = v;
        s_fb[px + 1] = v;
        s_fb[px + 2] = v;
    }
    esp_lcd_panel_draw_bitmap(s_panel, 0, 0, BOARD_PANEL_WIDTH_PX, BOARD_PANEL_HEIGHT_PX, s_fb);
    ESP_LOGI(TAG, "test pattern flushed");
}
