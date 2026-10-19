/*
 * ui.c — LVGL product UI (PRD §8): status bar + application launcher with
 * tappable tiles for the four MVP apps, plus per-app placeholder screens
 * that report real device status. The heavy app logic (VNC client, reader,
 * files) lives in the Rust crates and gets wired in via FFI later; this UI
 * is the touch-first shell they plug into.
 */
#include "ui.h"

#include "bsp/display.h"
#include "bsp/esp-bsp.h"
#include "esp_chip_info.h"
#include "esp_log.h"
#include "esp_system.h"
#include "lvgl.h"

static const char *UI_TAG = "p4-ui";

/* App identifiers — keep in sync with the Rust AppManager ids. */
typedef enum {
    APP_LAUNCHER = 0,
    APP_VNC,
    APP_READER,
    APP_FILES,
    APP_SETTINGS,
    APP_COUNT,
} app_id_t;

static const struct {
    const char *name;
    const char *subtitle;
    uint32_t color;
} app_defs[APP_COUNT] = {
    [APP_VNC]      = { "VNC",      "Remote desktop",       0x35507A },
    [APP_READER]   = { "Books",    "Ebook reader",         0x2E7D5B },
    [APP_FILES]    = { "Files",    "Browse storage",       0x8A6D1F },
    [APP_SETTINGS] = { "Settings", "Device & network",     0x5A5A66 },
};

static lv_obj_t *s_status_label;   /* right side of the status bar */
static lv_obj_t *s_root;           /* container swapped per screen */

/* ------------------------------------------------------------------ */
/* Small helpers                                                       */
/* ------------------------------------------------------------------ */

static lv_obj_t *screen_base(const char *title)
{
    lv_obj_t *scr = lv_obj_create(NULL);
    lv_obj_set_style_bg_color(scr, lv_color_hex(0x101418), LV_PART_MAIN);

    /* header with back button + title */
    lv_obj_t *hdr = lv_obj_create(scr);
    lv_obj_set_size(hdr, LV_PCT(100), 56);
    lv_obj_align(hdr, LV_ALIGN_TOP_MID, 0, 0);
    lv_obj_set_style_bg_color(hdr, lv_color_hex(0x1A2028), LV_PART_MAIN);
    lv_obj_set_style_border_width(hdr, 0, LV_PART_MAIN);
    lv_obj_set_style_radius(hdr, 0, LV_PART_MAIN);
    lv_obj_clear_flag(hdr, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t *back = lv_btn_create(hdr);
    lv_obj_set_size(back, 88, 40);
    lv_obj_align(back, LV_ALIGN_LEFT_MID, 8, 0);
    lv_obj_t *back_lbl = lv_label_create(back);
    lv_label_set_text(back_lbl, LV_SYMBOL_LEFT " Back");
    lv_obj_center(back_lbl);
    lv_obj_add_event_cb(back, ui_open_launcher_cb, LV_EVENT_CLICKED, NULL);

    lv_obj_t *title_lbl = lv_label_create(hdr);
    lv_label_set_text(title_lbl, title);
    lv_obj_set_style_text_font(title_lbl, &lv_font_montserrat_18, LV_PART_MAIN);
    lv_obj_align(title_lbl, LV_ALIGN_LEFT_MID, 110, 0);

    /* body container the screens fill in */
    s_root = lv_obj_create(scr);
    lv_obj_set_size(s_root, LV_PCT(100), LV_SIZE_CONTENT);
    lv_obj_align(s_root, LV_ALIGN_TOP_MID, 0, 64);
    lv_obj_set_style_bg_opa(s_root, LV_OPA_TRANSP, LV_PART_MAIN);
    lv_obj_set_style_border_width(s_root, 0, LV_PART_MAIN);

    lv_scr_load(scr);
    return s_root;
}

static void info_row(lv_obj_t *parent, const char *key, const char *value, int row)
{
    lv_obj_t *k = lv_label_create(parent);
    lv_label_set_text(k, key);
    lv_obj_set_style_text_color(k, lv_color_hex(0x8A94A0), LV_PART_MAIN);
    lv_obj_align(k, LV_ALIGN_TOP_LEFT, 0, row * 36);

    lv_obj_t *v = lv_label_create(parent);
    lv_label_set_text(v, value);
    lv_obj_align(v, LV_ALIGN_TOP_LEFT, 200, row * 36);
}

/* ------------------------------------------------------------------ */
/* App screens                                                         */
/* ------------------------------------------------------------------ */

static void show_vnc(void)
{
    lv_obj_t *body = screen_base("VNC — Remote desktop");
    lv_obj_t *lbl = lv_label_create(body);
    lv_label_set_text(lbl,
        "VNC client is not wired in this build.\n\n"
        "The RFB 3.8 protocol stack (handshake, DES auth,\n"
        "Raw/CopyRect decoders) is implemented and tested\n"
        "in the p4-vnc crate and connects here via FFI\n"
        "in an upcoming release.\n\n"
        "Wi-Fi bring-up lands first (ESP-Hosted on the C6).");
    lv_obj_set_style_text_color(lbl, lv_color_hex(0xC8D0DA), LV_PART_MAIN);
    lv_label_set_long_mode(lbl, LV_LABEL_LONG_WRAP);
    lv_obj_set_width(lbl, LV_PCT(90));
}

static void show_reader(void)
{
    lv_obj_t *body = screen_base("Books — Ebook reader");
    lv_obj_t *lbl = lv_label_create(body);
    lv_label_set_text(lbl,
        "The reader is not wired in this build.\n\n"
        "EPUB/TXT/Markdown parsing, pagination,\n"
        "bookmarks and resume are implemented and tested\n"
        "in the p4-reader crate; the UI lands together\n"
        "with SD card storage mount.");
    lv_obj_set_style_text_color(lbl, lv_color_hex(0xC8D0DA), LV_PART_MAIN);
    lv_label_set_long_mode(lbl, LV_LABEL_LONG_WRAP);
    lv_obj_set_width(lbl, LV_PCT(90));
}

static void show_files(void)
{
    lv_obj_t *body = screen_base("Files — Browse storage");
    lv_obj_t *lbl = lv_label_create(body);
    lv_label_set_text(lbl,
        "Storage is not mounted in this build.\n\n"
        "SD card (SDMMC) mount is the next bring-up step;\n"
        "after it lands this screen lists the card, and the\n"
        "p4-storage path sanitization guards all access.");
    lv_obj_set_style_text_color(lbl, lv_color_hex(0xC8D0DA), LV_PART_MAIN);
    lv_label_set_long_mode(lbl, LV_LABEL_LONG_WRAP);
    lv_obj_set_width(lbl, LV_PCT(90));
}

static void show_settings(void)
{
    lv_obj_t *body = screen_base("Settings — Device");

    char chip[64], mem[64];
    esp_chip_info_t info;
    esp_chip_info(&info);
    snprintf(chip, sizeof(chip), "ESP32-P4 rev %d.%d, %d cores",
             info.revision / 100, info.revision % 100, info.cores);
    uint32_t heap = esp_get_free_heap_size();
    snprintf(mem, sizeof(mem), "%lu KB free", (unsigned long)(heap / 1024));

    info_row(body, "Device",        "p4-client v0.4.0", 0);
    info_row(body, "Chip",          chip,               1);
    info_row(body, "Memory",        mem,                2);
    info_row(body, "Display",       "1024x600 MIPI-DSI (EK79007)", 3);
    info_row(body, "Touch",         "GT911 (via BSP)",  4);
    info_row(body, "Wi-Fi",         "bring-up pending", 5);
    info_row(body, "Storage",       "SD mount pending", 6);
}

/* ------------------------------------------------------------------ */
/* Launcher                                                            */
/* ------------------------------------------------------------------ */

static void app_tile_cb(lv_event_t *e)
{
    app_id_t id = (app_id_t)(intptr_t)lv_event_get_user_data(e);
    switch (id) {
    case APP_VNC:      show_vnc();      break;
    case APP_READER:   show_reader();   break;
    case APP_FILES:    show_files();    break;
    case APP_SETTINGS: show_settings(); break;
    default: break;
    }
}

static void build_status_bar(lv_obj_t *parent)
{
    lv_obj_t *bar = lv_obj_create(parent);
    lv_obj_set_size(bar, LV_PCT(100), 44);
    lv_obj_align(bar, LV_ALIGN_TOP_MID, 0, 0);
    lv_obj_set_style_bg_color(bar, lv_color_hex(0x1A2028), LV_PART_MAIN);
    lv_obj_set_style_border_width(bar, 0, LV_PART_MAIN);
    lv_obj_set_style_radius(bar, 0, LV_PART_MAIN);
    lv_obj_clear_flag(bar, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t *name = lv_label_create(bar);
    lv_label_set_text(name, "p4-client");
    lv_obj_set_style_text_color(name, lv_color_hex(0xE8EDF2), LV_PART_MAIN);
    lv_obj_align(name, LV_ALIGN_LEFT_MID, 12, 0);

    s_status_label = lv_label_create(bar);
    lv_label_set_text(s_status_label, "offline");
    lv_obj_set_style_text_color(s_status_label, lv_color_hex(0x8A94A0), LV_PART_MAIN);
    lv_obj_align(s_status_label, LV_ALIGN_RIGHT_MID, -12, 0);
}

void ui_open_launcher_cb(lv_event_t *e)
{
    (void)e;
    lv_obj_t *scr = lv_obj_create(NULL);
    lv_obj_set_style_bg_color(scr, lv_color_hex(0x101418), LV_PART_MAIN);
    lv_obj_clear_flag(scr, LV_OBJ_FLAG_SCROLLABLE);

    build_status_bar(scr);

    /* 2x2 tile grid, centered, generous 120 px hit targets (PRD §8) */
    int col_w = 380, row_h = 190, gap = 24;
    int grid_w = col_w * 2 + gap;
    int start_x = (BSP_LCD_H_RES - grid_w) / 2;
    int start_y = 44 + 90;

    for (int i = 0; i < APP_COUNT - 1; i++) {
        app_id_t id = (app_id_t)(i + 1);
        int col = i % 2, row = i / 2;

        lv_obj_t *tile = lv_obj_create(scr);
        lv_obj_set_size(tile, col_w, row_h);
        lv_obj_set_pos(tile, start_x + col * (col_w + gap), start_y + row * (row_h + gap));
        lv_obj_set_style_bg_color(tile, lv_color_hex(app_defs[id].color), LV_PART_MAIN);
        lv_obj_set_style_radius(tile, 16, LV_PART_MAIN);
        lv_obj_set_style_border_width(tile, 0, LV_PART_MAIN);
        lv_obj_set_style_shadow_width(tile, 40, LV_PART_MAIN);
        lv_obj_set_style_shadow_opa(tile, LV_OPA_30, LV_PART_MAIN);
        lv_obj_clear_flag(tile, LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_add_event_cb(tile, app_tile_cb, LV_EVENT_CLICKED, (void *)(intptr_t)id);

        lv_obj_t *icon = lv_label_create(tile);
        lv_label_set_text(icon,
            id == APP_VNC      ? LV_SYMBOL_WIFI :
            id == APP_READER   ? LV_SYMBOL_FILE :
            id == APP_FILES    ? LV_SYMBOL_DIRECTORY : LV_SYMBOL_SETTINGS);
        lv_obj_set_style_text_font(icon, &lv_font_montserrat_22, LV_PART_MAIN);
        lv_obj_align(icon, LV_ALIGN_TOP_LEFT, 20, 18);

        lv_obj_t *name = lv_label_create(tile);
        lv_label_set_text(name, app_defs[id].name);
        lv_obj_set_style_text_font(name, &lv_font_montserrat_18, LV_PART_MAIN);
        lv_obj_set_style_text_color(name, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
        lv_obj_align(name, LV_ALIGN_BOTTOM_LEFT, 20, -34);

        lv_obj_t *sub = lv_label_create(tile);
        lv_label_set_text(sub, app_defs[id].subtitle);
        lv_obj_set_style_text_color(sub, lv_color_hex(0xC8D0DA), LV_PART_MAIN);
        lv_obj_align(sub, LV_ALIGN_BOTTOM_LEFT, 20, -12);
    }

    lv_scr_load(scr);
    ESP_LOGI(UI_TAG, "launcher loaded");
}

void ui_init(void)
{
    ui_open_launcher_cb(NULL);
}
