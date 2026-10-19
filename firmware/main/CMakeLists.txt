idf_component_register(
    SRCS "main.c" "ui.c"
    INCLUDE_DIRS "."
    REQUIRES esp32_p4_wifi6_touch_lcd_7b lvgl__lvgl nvs_flash)
