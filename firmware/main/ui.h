/* ui.h — LVGL product UI: status bar + launcher + app screens. */
#pragma once

#include "lvgl.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Build and load the launcher screen. Requires the BSP display/lock. */
void ui_init(void);

/** Event callback (also used by per-screen Back buttons). */
void ui_open_launcher_cb(lv_event_t *e);

#ifdef __cplusplus
}
#endif
