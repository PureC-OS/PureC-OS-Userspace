#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void audio_panel_init(void);
void audio_panel_draw(uint32_t screen_width);
void audio_panel_draw_badge(uint32_t screen_width);
void audio_panel_draw_popup(uint32_t screen_width);
void audio_panel_popup_rect(uint32_t screen_width, uint32_t *x, uint32_t *y,
                            uint32_t *w, uint32_t *h);
void audio_panel_set_popup(bool visible);
bool audio_panel_handle_mouse(
    int32_t x,
    int32_t y,
    uint8_t buttons,
    bool pressed,
    bool released,
    uint32_t screen_width,
    bool *redraw_required
);
bool audio_panel_handle_special_key(uint8_t key);
bool audio_panel_is_popup_visible(void);

#ifdef __cplusplus
}
#endif
