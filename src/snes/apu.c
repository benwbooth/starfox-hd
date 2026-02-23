#include "apu.h"
#include "../audio/audio.h"
#include <string.h>

// APU I/O ports (bidirectional communication with SPC700)
static uint8 s_ports_to_spc[4];   // CPU → SPC700
static uint8 s_ports_from_spc[4]; // SPC700 → CPU

void Apu_Init(void) {
    memset(s_ports_to_spc, 0, sizeof(s_ports_to_spc));
    memset(s_ports_from_spc, 0, sizeof(s_ports_from_spc));
}

void Apu_WritePort(int port, uint8 val) {
    if (port >= 0 && port < 4) {
        s_ports_to_spc[port] = val;
        // Forward commands to audio subsystem
        if (port == 0) {
            Audio_SendCommand(val, s_ports_to_spc[1]);
        }
    }
}

uint8 Apu_ReadPort(int port) {
    if (port >= 0 && port < 4) {
        return s_ports_from_spc[port];
    }
    return 0;
}
