/* touch.h — GT911 touch init, registered with the LVGL port. */
#pragma once

#include "esp_err.h"
#include "lvgl.h"

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t touch_init(lv_display_t *display, lv_indev_t **out_indev);

#ifdef __cplusplus
}
#endif
