idf_component_register(
    SRCS "main.c" "ui.c" "touch.c"
    INCLUDE_DIRS "."
    PRIV_REQUIRES display board esp_lcd driver nvs_flash esp_lvgl_port)
