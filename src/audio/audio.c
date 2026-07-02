#include "audio.h"
#include "spc_player.h"
#include "spc_boot.h"
#include <SDL2/SDL.h>
#include <stdio.h>
#include <stdlib.h>

static SDL_AudioDeviceID s_audio_dev = 0;
static int s_volume = 100;

// ---------------------------------------------------------------------------
// Debug WAV dump of the audio callback stream (SF_AUDIO_WAV=<path>).
// Header is (re)finalized on shutdown.
// ---------------------------------------------------------------------------
static FILE   *s_wav_file = NULL;
static uint32  s_wav_bytes = 0;

static void wav_write_header(FILE *f, uint32 data_bytes) {
    uint32 u32;
    uint16 u16;
    fseek(f, 0, SEEK_SET);
    fwrite("RIFF", 1, 4, f);
    u32 = 36 + data_bytes;        fwrite(&u32, 4, 1, f);
    fwrite("WAVEfmt ", 1, 8, f);
    u32 = 16;                     fwrite(&u32, 4, 1, f);
    u16 = 1;                      fwrite(&u16, 2, 1, f);  // PCM
    u16 = 2;                      fwrite(&u16, 2, 1, f);  // stereo
    u32 = 32000;                  fwrite(&u32, 4, 1, f);  // sample rate
    u32 = 32000 * 4;              fwrite(&u32, 4, 1, f);  // byte rate
    u16 = 4;                      fwrite(&u16, 2, 1, f);  // block align
    u16 = 16;                     fwrite(&u16, 2, 1, f);  // bits/sample
    fwrite("data", 1, 4, f);
    u32 = data_bytes;             fwrite(&u32, 4, 1, f);
}

static void AudioCallback(void *userdata, uint8 *stream, int len) {
    (void)userdata;
    // len is in bytes; each frame = 2 channels * 2 bytes = 4 bytes
    int num_samples = len / 4;
    SpcPlayer_Generate((int16 *)stream, num_samples);

    // Apply master volume
    if (s_volume < 100) {
        int16 *samples = (int16 *)stream;
        int count = len / 2;
        for (int i = 0; i < count; i++) {
            samples[i] = (int16)((int32)samples[i] * s_volume / 100);
        }
    }

    if (s_wav_file) {
        fwrite(stream, 1, (size_t)len, s_wav_file);
        s_wav_bytes += (uint32)len;
    }
}

void Audio_Init(const Config *cfg) {
    s_volume = cfg->volume;

    // Optional debug WAV capture of everything sent to the audio device.
    {
        const char *wav_path = getenv("SF_AUDIO_WAV");
        if (wav_path && wav_path[0]) {
            s_wav_file = fopen(wav_path, "wb");
            if (s_wav_file) {
                wav_write_header(s_wav_file, 0);
                printf("Audio: dumping callback stream to %s\n", wav_path);
            } else {
                fprintf(stderr, "Audio: cannot open SF_AUDIO_WAV file %s\n", wav_path);
            }
        }
    }

    // Request SPC native sample rate (32000 Hz).  SDL will resample
    // internally if the audio hardware doesn't support it natively.
    SDL_AudioSpec want, have;
    SDL_memset(&want, 0, sizeof(want));
    want.freq     = 32000;    // SPC700 native rate
    want.format   = AUDIO_S16SYS;
    want.channels = 2;
    want.samples  = cfg->buffer_size;
    want.callback = AudioCallback;

    s_audio_dev = SDL_OpenAudioDevice(NULL, 0, &want, &have, 0);
    if (s_audio_dev == 0) {
        fprintf(stderr, "Audio: failed to open device: %s\n", SDL_GetError());
        return;
    }

    // Init SPC emulator (sample_rate arg is informational; SPC always runs at 32kHz)
    SpcPlayer_Init(have.freq);

    // Install IPL ROM and boot the SPC driver
    SpcBoot_SetAssetDir(cfg->asset_dir);
    SpcBoot_Init();
    SpcPlayer_LoadTrack(SND_INIT);

    // Start playback
    SDL_PauseAudioDevice(s_audio_dev, 0);
    printf("Audio initialized: %d Hz, %d buffer, volume %d%%\n",
           have.freq, have.samples, s_volume);
}

void Audio_Shutdown(void) {
    if (s_audio_dev) {
        SDL_CloseAudioDevice(s_audio_dev);
        s_audio_dev = 0;
    }
    if (s_wav_file) {
        wav_write_header(s_wav_file, s_wav_bytes);
        fclose(s_wav_file);
        s_wav_file = NULL;
        printf("Audio: WAV dump finalized (%u bytes of PCM)\n", (unsigned)s_wav_bytes);
    }
    SpcPlayer_Shutdown();
}

void Audio_SendCommand(uint8 cmd, uint8 param) {
    SpcPlayer_SendCommand(cmd, param);
}

void Audio_SetVolume(int volume) {
    if (volume < 0) volume = 0;
    if (volume > 100) volume = 100;
    s_volume = volume;
}
