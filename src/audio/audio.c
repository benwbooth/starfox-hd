#include "audio.h"
#include "spc_player.h"
#include <SDL2/SDL.h>
#include <stdio.h>

static SDL_AudioDeviceID s_audio_dev = 0;
static int s_sample_rate = 48000;
static int s_volume = 100;

static void AudioCallback(void *userdata, uint8 *stream, int len) {
    (void)userdata;
    SpcPlayer_Generate((int16 *)stream, len / 4);

    // Apply volume
    if (s_volume < 100) {
        int16 *samples = (int16 *)stream;
        int count = len / 2;
        for (int i = 0; i < count; i++) {
            samples[i] = (int16)((int32)samples[i] * s_volume / 100);
        }
    }
}

void Audio_Init(const Config *cfg) {
    s_sample_rate = cfg->sample_rate;
    s_volume = cfg->volume;

    SDL_AudioSpec want, have;
    SDL_memset(&want, 0, sizeof(want));
    want.freq = s_sample_rate;
    want.format = AUDIO_S16SYS;
    want.channels = 2;
    want.samples = cfg->buffer_size;
    want.callback = AudioCallback;

    s_audio_dev = SDL_OpenAudioDevice(NULL, 0, &want, &have, 0);
    if (s_audio_dev == 0) {
        fprintf(stderr, "Audio: failed to open device: %s\n", SDL_GetError());
        return;
    }

    SpcPlayer_Init(have.freq);

    // Start playback
    SDL_PauseAudioDevice(s_audio_dev, 0);
    printf("Audio initialized: %dHz, %d buffer\n", have.freq, have.samples);
}

void Audio_Shutdown(void) {
    if (s_audio_dev) {
        SDL_CloseAudioDevice(s_audio_dev);
        s_audio_dev = 0;
    }
    SpcPlayer_Shutdown();
}

void Audio_SendCommand(uint8 cmd, uint8 param) {
    SpcPlayer_SendCommand(cmd, param);
}

void Audio_SetVolume(int volume) {
    s_volume = CLAMP(volume, 0, 100);
}
