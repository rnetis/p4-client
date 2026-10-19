/* P4 Client firmware entry point (PRD §7 boot sequence).
 *
 * Power On -> Hardware Init -> Display Init -> [Storage/Wi-Fi: Phase 0 TODO]
 *
 * v0.2: real MIPI-DSI display bring-up. After display_init() the panel shows
 * a solid color, then the bring-up test pattern (color bars + grayscale
 * gradient). If display init fails, the error is logged and the boot stops
 * there instead of hanging silently (PRD §24: log everything to serial).
 *
 * Recovery mode is entered when the BOOT button is held at reset (PRD §23).
 */

#include <stdio.h>

#include "display.h"
#include "esp_err.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "nvs_flash.h"

static const char *TAG = "p4-boot";

typedef enum {
    BOOT_HW = 0,
    BOOT_DISPLAY,
    BOOT_STORAGE,
    BOOT_WIFI,
    BOOT_CONFIG,
    BOOT_UI,
} boot_stage_t;

static volatile boot_stage_t s_boot_stage = BOOT_HW;

static void hardware_init(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ESP_ERROR_CHECK(nvs_flash_init());
    }
    ESP_LOGI(TAG, "hardware init done");
}

/* Boot loop: on display failure, blink the log every 5 s so a stuck console
 * is distinguishable from a hung SoC. */
void app_main(void)
{
    ESP_LOGI(TAG, "P4 Client boot v%s (stage %d)", "0.2.0", s_boot_stage);

    hardware_init();                     /* BOOT_HW */

    s_boot_stage = BOOT_DISPLAY;
    ESP_LOGI(TAG, "display init (MIPI-DSI, EK79007 1024x600)...");
    esp_err_t disp = display_init();
    if (disp != ESP_OK) {
        ESP_LOGE(TAG, "DISPLAY INIT FAILED: %s — boot halted at stage %d",
                 esp_err_to_name(disp), s_boot_stage);
        while (1) {
            vTaskDelay(pdMS_TO_TICKS(5000));
            ESP_LOGE(TAG, "boot halted (display init failed)");
        }
    }
    vTaskDelay(pdMS_TO_TICKS(1000));     /* let the solid color be visible */
    display_show_test_pattern();

    /* Hardware-pattern diagnostics (Waveshare example style): vertical DSI
     * bars for 3 s (proves PHY/lanes independent of panel init), then back
     * to the framebuffer pattern. */
    if (display_hw_pattern(true) == ESP_OK) {
        ESP_LOGI(TAG, "DSI hardware pattern: vertical bars for 3 s");
        vTaskDelay(pdMS_TO_TICKS(3000));
        display_hw_pattern(false);
        display_show_test_pattern();     /* restore framebuffer pattern */
        ESP_LOGI(TAG, "back to framebuffer test pattern");
    } else {
        ESP_LOGW(TAG, "DSI hardware pattern not supported by this driver");
    }

    s_boot_stage = BOOT_STORAGE;
    ESP_LOGW(TAG, "storage init: TODO (PRD Phase 0 — SD/FAT mount)");
    s_boot_stage = BOOT_WIFI;
    ESP_LOGW(TAG, "wifi init: TODO (PRD Phase 0 — ESP-Hosted on C6)");
    s_boot_stage = BOOT_CONFIG;
    ESP_LOGW(TAG, "config load: TODO (PRD Phase 0 — settings from NVS)");
    s_boot_stage = BOOT_UI;
    ESP_LOGW(TAG, "ui start: TODO (LVGL task + Rust app layer)");

    ESP_LOGI(TAG, "boot complete through stage %d; display test pattern on screen",
             s_boot_stage);

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
        ESP_LOGI(TAG, "alive (stage %d)", s_boot_stage);
    }
}
