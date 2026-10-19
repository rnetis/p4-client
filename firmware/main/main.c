/* P4 Client firmware entry point (PRD §7 boot sequence).
 *
 * v0.4.0: product UI. Boots into the LVGL launcher (status bar + app tiles
 * for VNC / Books / Files / Settings) with GT911 touch input via the
 * Waveshare BSP. Display bring-up was hardware-validated in v0.2–v0.3; the
 * BSP now owns display/touch/backlight. Storage, Wi-Fi and the Rust app
 * layer (VNC/reader/files logic) wire into the UI in upcoming releases.
 *
 * Recovery mode is entered when the BOOT button is held at reset (PRD §23).
 */

#include <stdio.h>

#include "bsp/esp-bsp.h"
#include "bsp/display.h"
#include "esp_err.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include "ui.h"

static const char *TAG = "p4-boot";

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
    ESP_LOGI(TAG, "P4 Client v0.4.0 boot");

    hardware_init();

    /* Display + touch via the Waveshare BSP (validated config):
     * triple-buffer partial refresh, GT911 mirrored for panel orientation. */
    bsp_display_cfg_t cfg = {
        .lv_adapter_cfg = ESP_LV_ADAPTER_DEFAULT_CONFIG(),
        .rotation = ESP_LV_ADAPTER_ROTATE_0,
        .tear_avoid_mode = ESP_LV_ADAPTER_TEAR_AVOID_MODE_TRIPLE_PARTIAL,
        .touch_flags = {
            .swap_xy = 0,
            .mirror_x = 1,
            .mirror_y = 1,
        },
    };
    lv_display_t *display = bsp_display_start_with_config(&cfg);
    if (display == NULL) {
        ESP_LOGE(TAG, "display init failed — boot halted");
        while (1) {
            vTaskDelay(pdMS_TO_TICKS(5000));
            ESP_LOGE(TAG, "boot halted (display init failed)");
        }
    }
    bsp_display_backlight_on();
    ESP_LOGI(TAG, "display online via BSP, backlight on");

    ESP_ERROR_CHECK(bsp_display_lock(-1) ? ESP_OK : ESP_ERR_TIMEOUT);
    ui_init();
    bsp_display_unlock();

    ESP_LOGI(TAG, "launcher up; storage/wifi/rust-app wiring pending");

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(30000));
        ESP_LOGI(TAG, "alive");
    }
}
