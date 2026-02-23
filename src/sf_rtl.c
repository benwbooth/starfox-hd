#include "sf_rtl.h"
#include <string.h>
#include <stdio.h>

// SNES Memory
uint8 g_ram[WRAM_SIZE];
uint8 g_sram[SRAM_SIZE];
uint8 g_vram[VRAM_SIZE];

// Controller state
uint16 g_pad1 = 0;
uint16 g_pad1_new = 0;
uint16 g_pad1_prev = 0;

// PRNG state
uint16 g_rndval = 0;

// Frame counter
uint32 g_frame_count = 0;

// Hardware multiply/divide state
static uint16 s_div_remainder = 0;

// Keyboard → SNES button mapping
static const struct {
    SDL_Scancode key;
    uint16 pad_bit;
} key_map[] = {
    { SDL_SCANCODE_X,      PAD_A },       // A button
    { SDL_SCANCODE_Z,      PAD_B },       // B button
    { SDL_SCANCODE_A,      PAD_X },       // X button
    { SDL_SCANCODE_S,      PAD_Y },       // Y button
    { SDL_SCANCODE_Q,      PAD_TLEFT },   // L shoulder
    { SDL_SCANCODE_W,      PAD_TRIGHT },  // R shoulder
    { SDL_SCANCODE_RETURN, PAD_START },   // Start
    { SDL_SCANCODE_RSHIFT, PAD_SELECT },  // Select
    { SDL_SCANCODE_UP,     PAD_UP },
    { SDL_SCANCODE_DOWN,   PAD_DOWN },
    { SDL_SCANCODE_LEFT,   PAD_LEFT },
    { SDL_SCANCODE_RIGHT,  PAD_RIGHT },
};
#define KEY_MAP_COUNT (sizeof(key_map) / sizeof(key_map[0]))

// Gamepad state
static SDL_GameController *s_controller = NULL;

void SfRtl_Init(void) {
    memset(g_ram, 0, sizeof(g_ram));
    memset(g_sram, 0, sizeof(g_sram));
    memset(g_vram, 0, sizeof(g_vram));
    g_rndval = 0;
    g_frame_count = 0;

    // Open first available game controller
    for (int i = 0; i < SDL_NumJoysticks(); i++) {
        if (SDL_IsGameController(i)) {
            s_controller = SDL_GameControllerOpen(i);
            if (s_controller) {
                printf("Controller: %s\n", SDL_GameControllerName(s_controller));
                break;
            }
        }
    }
}

void SfRtl_HandleEvent(const SDL_Event *event) {
    if (event->type == SDL_CONTROLLERDEVICEADDED && !s_controller) {
        s_controller = SDL_GameControllerOpen(event->cdevice.which);
        if (s_controller) {
            printf("Controller connected: %s\n", SDL_GameControllerName(s_controller));
        }
    } else if (event->type == SDL_CONTROLLERDEVICEREMOVED && s_controller) {
        SDL_GameControllerClose(s_controller);
        s_controller = NULL;
        printf("Controller disconnected\n");
    }
}

static uint16 ReadKeyboard(void) {
    const uint8 *keys = SDL_GetKeyboardState(NULL);
    uint16 pad = 0;
    for (int i = 0; i < (int)KEY_MAP_COUNT; i++) {
        if (keys[key_map[i].key]) {
            pad |= key_map[i].pad_bit;
        }
    }
    return pad;
}

static uint16 ReadGamepad(void) {
    if (!s_controller) return 0;
    uint16 pad = 0;

    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_A))          pad |= PAD_A;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_B))          pad |= PAD_B;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_X))          pad |= PAD_X;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_Y))          pad |= PAD_Y;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_LEFTSHOULDER))  pad |= PAD_TLEFT;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_RIGHTSHOULDER)) pad |= PAD_TRIGHT;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_START))      pad |= PAD_START;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_BACK))       pad |= PAD_SELECT;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_DPAD_UP))    pad |= PAD_UP;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_DPAD_DOWN))  pad |= PAD_DOWN;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_DPAD_LEFT))  pad |= PAD_LEFT;
    if (SDL_GameControllerGetButton(s_controller, SDL_CONTROLLER_BUTTON_DPAD_RIGHT)) pad |= PAD_RIGHT;

    // Analog stick → dpad with deadzone
    int16 lx = SDL_GameControllerGetAxis(s_controller, SDL_CONTROLLER_AXIS_LEFTX);
    int16 ly = SDL_GameControllerGetAxis(s_controller, SDL_CONTROLLER_AXIS_LEFTY);
    const int16 DEADZONE = 8000;
    if (lx < -DEADZONE) pad |= PAD_LEFT;
    if (lx >  DEADZONE) pad |= PAD_RIGHT;
    if (ly < -DEADZONE) pad |= PAD_UP;
    if (ly >  DEADZONE) pad |= PAD_DOWN;

    return pad;
}

void SfRtl_BeginFrame(void) {
    g_pad1_prev = g_pad1;
    g_pad1 = ReadKeyboard() | ReadGamepad();
    g_pad1_new = g_pad1 & ~g_pad1_prev;
    g_frame_count++;
}

// PPU stubs — for 3D gameplay these are no-ops
void SfRtl_PpuWrite(uint16 reg, uint8 val) {
    (void)reg; (void)val;
}

uint8 SfRtl_PpuRead(uint16 reg) {
    (void)reg;
    return 0;
}

// DMA — becomes a memory copy
void SfRtl_DmaTransfer(int channel, uint32 src, uint16 dst, uint16 size) {
    (void)channel;
    // Source from ROM/RAM, destination to VRAM
    // For now, just handle RAM→VRAM copies
    if (src >= 0x7E0000 && src < 0x7E0000 + WRAM_SIZE) {
        uint32 ram_offset = src - 0x7E0000;
        if (ram_offset + size <= WRAM_SIZE && dst + size <= VRAM_SIZE) {
            memcpy(&g_vram[dst], &g_ram[ram_offset], size);
        }
    }
}

// Hardware multiply: $4202 * $4203
uint16 SfRtl_Multiply(uint8 a, uint8 b) {
    return (uint16)a * (uint16)b;
}

// Hardware divide: $4204-$4205 / $4206
uint16 SfRtl_Divide(uint16 dividend, uint8 divisor) {
    if (divisor == 0) {
        s_div_remainder = dividend;
        return 0xFFFF;
    }
    s_div_remainder = dividend % divisor;
    return dividend / divisor;
}

uint16 SfRtl_DivRemainder(void) {
    return s_div_remainder;
}

// PRNG — exact match to SNES
uint16 SfRtl_Random(void) {
    g_rndval = PRNG_NEXT(g_rndval);
    return g_rndval;
}
