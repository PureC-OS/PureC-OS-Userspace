#include "ring3.h"
#include "purec.h"

void panel_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h,
                     uint32_t color) {
    pc_draw_rect(x, y, w, h, color);
}

void panel_draw_text(uint32_t x, uint32_t y, const char *text, uint32_t fg,
                     uint32_t bg) {
    pc_draw_text(x, y, text, fg, bg);
}

bool panel_audio_get_status(struct panel_audio_status *status) {
    return pc_audio_get_status((struct audio_status *)status);
}

uint8_t panel_audio_get_volume(void) {
    int32_t volume = pc_audio_get_volume();
    if (volume < 0) {
        return 0;
    }
    if (volume > 100) {
        return 100;
    }
    return (uint8_t)volume;
}

bool panel_audio_is_muted(void) {
    return pc_audio_is_muted();
}

void panel_audio_set_volume(uint8_t volume) {
    pc_audio_set_volume(volume);
}

void panel_audio_set_muted(bool muted) {
    pc_audio_set_muted(muted);
}

void panel_audio_toggle_mute(void) {
    panel_audio_set_muted(!panel_audio_is_muted());
}

void panel_audio_adjust_volume(int8_t delta) {
    pc_audio_adjust_volume(delta);
}

bool panel_audio_select_output_device(uint32_t index) {
    return pc_audio_select_output(index);
}

void panel_audio_play_test_sound(void) {
    pc_audio_play_test();
}
