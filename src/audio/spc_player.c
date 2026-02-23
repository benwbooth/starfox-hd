#include "spc_player.h"
#include <string.h>
#include <stdio.h>

// TODO: Integrate blargg's snes_spc library
// For now, generate silence

static int s_sample_rate = 48000;
static bool s_initialized = false;

void SpcPlayer_Init(int sample_rate) {
    s_sample_rate = sample_rate;
    s_initialized = true;
    printf("SPC Player: stub initialized at %dHz (audio silent until snes_spc integrated)\n",
           sample_rate);
}

void SpcPlayer_Shutdown(void) {
    s_initialized = false;
}

bool SpcPlayer_LoadFromRom(const uint8 *spc_data, int spc_size,
                            const uint8 *brr_data, int brr_size) {
    (void)spc_data; (void)spc_size;
    (void)brr_data; (void)brr_size;
    // TODO: Load into snes_spc emulator
    return false;
}

void SpcPlayer_Generate(int16 *buffer, int num_samples) {
    if (!s_initialized) {
        memset(buffer, 0, num_samples * 2 * sizeof(int16));
        return;
    }
    // TODO: Call snes_spc to generate samples
    memset(buffer, 0, num_samples * 2 * sizeof(int16));
}

void SpcPlayer_SendCommand(uint8 cmd, uint8 param) {
    (void)cmd; (void)param;
    // TODO: Write to SPC700 APU ports
}
