/* P4 Client firmware entry point (PRD §7 boot sequence).
 *
 * Power On -> Hardware Init -> Display Init -> Storage Init
 *          -> Wi-Fi Init -> Load Config -> Launch UI
 *
 * This is the ESP-IDF bring-up skeleton. Each stage logs to the serial
 * console (PRD §24) and sets a boot-progress flag consumed by the Rust
 * application layer once it is linked in via esp-idf-sys. Recovery mode is
 * entered when the BOOT button is held at reset (PRD §23).
 */

#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_system.h"
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

void app_main(void)
{
    ESP_LOGI(TAG, "P4 Client boot (stage %d)", s_boot_stage);

    hardware_init();          /* BOOT_HW */
    s_boot_stage = BOOT_DISPLAY;
    /* display_init();        TODO: MIPI-DSI panel bring-up on real board */
    s_boot_stage = BOOT_STORAGE;
    /* storage_init();        TODO: SD/FAT mount */
    s_boot_stage = BOOT_WIFI;
    /* wifi_init();           TODO: ESP-Hosted (C6) */
    s_boot_stage = BOOT_CONFIG;
    /* config_load();         TODO: settings from NVS */
    s_boot_stage = BOOT_UI;
    /* ui_start();            TODO: start LVGL task, hand off to Rust app layer */

    ESP_LOGI(TAG, "boot skeleton complete; UI bring-up pending (PRD Phase 0)");

    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
}
