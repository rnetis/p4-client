/* GT911 capacitive touch on I2C (GPIO8=SCL, GPIO7=SDA).
 * Registered with the esp_lvgl_port so LVGL receives pointer events.
 */
#include "touch.h"

#include "board.h"
#include "driver/i2c_master.h"
#include "esp_lcd_touch_gt911.h"
#include "esp_log.h"
#include "esp_lvgl_port_touch.h"

static const char *TAG = "p4-touch";

esp_err_t touch_init(lv_display_t *display, lv_indev_t **out_indev)
{
    esp_err_t err;

    /* I2C master on the board's touch bus */
    const i2c_master_bus_config_t i2c_cfg = {
        .i2c_port = 0,
        .sda_io_num = GPIO_NUM_7,
        .scl_io_num = GPIO_NUM_8,
        .clk_source = I2C_CLK_SRC_DEFAULT,
        .glitch_ignore_cnt = 7,
        .flags.enable_internal_pullup = true,
    };
    i2c_master_bus_handle_t i2c_bus;
    err = i2c_new_master_bus(&i2c_cfg, &i2c_bus);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "I2C bus init failed: %s", esp_err_to_name(err));
        return err;
    }

    /* GT911: probe 0x5D then 0x14 (address depends on the panel's strap) */
    esp_lcd_panel_io_handle_t tp_io;
    const esp_lcd_panel_io_i2c_config_t tp_io_cfg =
        ESP_LCD_TOUCH_IO_I2C_GT911_CONFIG();
    err = esp_lcd_new_panel_io_i2c(i2c_bus, &tp_io_cfg, &tp_io);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "touch I2C io init failed: %s", esp_err_to_name(err));
        return err;
    }

    const esp_lcd_touch_config_t tp_cfg = {
        .x_max = BOARD_PANEL_WIDTH_PX,
        .y_max = BOARD_PANEL_HEIGHT_PX,
        .rst_gpio_num = GPIO_NUM_NC,
        .int_gpio_num = GPIO_NUM_NC,
        .flags = {
            .swap_xy = 0,
            .mirror_x = 1,
            .mirror_y = 1,
        },
    };
    esp_lcd_touch_handle_t tp;
    err = esp_lcd_touch_new_i2c_gt911(tp_io, &tp_cfg, &tp);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "GT911 init failed (probed both addresses): %s",
                 esp_err_to_name(err));
        return err;
    }

    const lvgl_port_touch_cfg_t touch_cfg = {
        .disp = display,
        .handle = tp,
    };
    lv_indev_t *indev = lvgl_port_add_touch(&touch_cfg);
    if (indev == NULL) {
        ESP_LOGE(TAG, "LVGL touch registration failed");
        return ESP_FAIL;
    }
    *out_indev = indev;
    ESP_LOGI(TAG, "GT911 touch ready, registered with LVGL");
    return ESP_OK;
}
