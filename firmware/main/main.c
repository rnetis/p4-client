/* P4 Client firmware entry point (PRD §7 boot sequence).
 *
 * v0.4.1: back to the hardware-validated display bring-up (components/
 * display — the exact sequence that showed the test pattern on this board),
 * with LVGL attached on top via esp_lvgl_port and GT911 touch on I2C.
 * No BSP in the display path: every layer below LVGL is proven.
 *
 * Boot sequence: NVS -> LDO(DSI PHY) -> DSI bus -> EK79007 panel ->
 * framebuffer -> backlight -> LVGL port -> GT911 touch -> launcher UI.
 */

#include <stdio.h>

#include "display.h"
#include "esp_err.h"
#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "esp_lvgl_port.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include "touch.h"
#include "ui.h"

static const char *TAG = "p4-boot";

/* Panel handle shared with the LVGL port (set by display bring-up). */
extern esp_lcd_panel_handle_t p4_panel_handle(void);

static void hardware_init(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ESP_ERROR_CHECK(nvs_flash_init());
    }
    ESP_LOGI(TAG, "hardware init done (NVS)");
}

void app_main(void)
{
    ESP_LOGI(TAG, "P4 Client v0.4.1 boot");

    hardware_init();

    /* 1. Validated display bring-up: LDO -> DSI -> EK79007 -> framebuffer.
     *    Leaves the panel powered with a solid dark blue test color. */
    ESP_ERROR_CHECK(display_init());

    /* 2. LVGL port on top of the proven esp_lcd panel handle. */
    const lvgl_port_cfg_t lvgl_cfg = ESP_LVGL_PORT_INIT_CONFIG();
    ESP_ERROR_CHECK(lvgl_port_init(&lvgl_cfg));

    lvgl_port_display_cfg_t disp_cfg = {
        .panel_handle = p4_panel_handle(),
        .buffer_size = 1024 * 60,          /* 1/10 frame, RGB888, in PSRAM */
        .double_buffer = true,
        .hres = 1024,
        .vres = 600,
        .color_format = 0,                 /* auto: follow the DPI panel's format */
        .rotation = {
            .swap_xy = false,
            .mirror_x = false,
            .mirror_y = false,
        },
        .flags = {
            .swap_bytes = false,
            .buff_dma = false,             /* PSRAM, DPI panel owns its FB */
            .buff_spiram = true,
        },
    };
    lv_display_t *lvgl_disp = lvgl_port_add_disp(&disp_cfg);
    if (lvgl_disp == NULL) {
        ESP_LOGE(TAG, "LVGL display registration failed — boot halted");
        while (1) vTaskDelay(pdMS_TO_TICKS(5000));
    }

    /* 3. GT911 touch -> LVGL indev */
    lv_indev_t *indev;
    ESP_ERROR_CHECK(touch_init(lvgl_disp, &indev));

    /* 4. Product UI */
    ESP_ERROR_CHECK(lvgl_port_lock(0) ? ESP_OK : ESP_ERR_TIMEOUT);
    ui_init();
    lvgl_port_unlock();

    ESP_LOGI(TAG, "launcher up (touch enabled); storage/wifi/rust wiring pending");

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(30000));
        ESP_LOGI(TAG, "alive");
    }
}
