#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void panel_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h,
                     uint32_t color);
void panel_draw_text(uint32_t x, uint32_t y, const char *text, uint32_t fg,
                     uint32_t bg);

struct panel_audio_status {
    uint32_t volume;
    uint32_t muted;
    uint32_t backend;
    uint32_t available_backends;
    uint32_t pcm_ready;
    uint32_t test_active;
    uint32_t output_device_count;
    uint32_t selected_output_device;
    uint32_t hda_codec;
    uint32_t hda_dac_node;
    uint32_t hda_pin_node;
};

bool panel_audio_get_status(struct panel_audio_status *status);
uint8_t panel_audio_get_volume(void);
bool panel_audio_is_muted(void);
void panel_audio_set_volume(uint8_t volume);
void panel_audio_set_muted(bool muted);
void panel_audio_toggle_mute(void);
void panel_audio_adjust_volume(int8_t delta);
bool panel_audio_select_output_device(uint32_t index);
void panel_audio_play_test_sound(void);

#define display_draw_rect panel_draw_rect
#define display_draw_text_at panel_draw_text
#define audio_status panel_audio_status
#define AUDIO_BACKEND_HDA 2
#define AUDIO_BACKEND_PC_SPEAKER 1
#define userspace_audio_get_status panel_audio_get_status
#define userspace_audio_get_volume panel_audio_get_volume
#define userspace_audio_is_muted panel_audio_is_muted
#define userspace_audio_set_volume panel_audio_set_volume
#define userspace_audio_set_muted panel_audio_set_muted
#define userspace_audio_toggle_mute panel_audio_toggle_mute
#define userspace_audio_adjust_volume panel_audio_adjust_volume
#define userspace_audio_select_output_device panel_audio_select_output_device
#define userspace_audio_play_test_sound panel_audio_play_test_sound

#ifdef __cplusplus
}
#endif
