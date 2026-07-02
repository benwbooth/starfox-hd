// SPC700 audio emulation via blargg's snes_spc library
//
// The Star Fox SPC sound driver runs inside the emulator.  The game thread
// communicates with it via APU port writes (engine sounds on port 1, nearest-
// object sounds on port 2, SFX on port 3).  Because the SDL audio callback
// runs on a separate thread, all port writes are queued and flushed at the
// start of each audio callback invocation.

#include "spc_player.h"
#include "spc_boot.h"
#include <spc.h>
#include <SDL2/SDL.h>
#include <string.h>
#include <stdio.h>

// ---------------------------------------------------------------------------
// SPC emulator state
// ---------------------------------------------------------------------------
static SNES_SPC  *s_spc    = NULL;
static SPC_Filter *s_filter = NULL;

// ---------------------------------------------------------------------------
// Port write queue  (game thread -> audio thread)
// ---------------------------------------------------------------------------
#define PORT_QUEUE_SIZE 128

typedef struct {
    uint8 port;
    uint8 value;
} PortWrite;

static PortWrite s_port_queue[PORT_QUEUE_SIZE];
static int       s_pq_head = 0;   // written by game thread (under lock)
static int       s_pq_tail = 0;   // consumed by audio thread (under lock)

static SDL_mutex *s_mutex = NULL;

// ---------------------------------------------------------------------------
// BGM start handshake (startmus in IRQ.ASM)
//
// The original runs this in the IRQ handler every frame regardless of game
// state, so it lives here on the always-running audio thread rather than in
// the per-state game tick.  State: 0 = command pending, 1 = sent awaiting
// the driver's echo on port 0, 2 = idle.
// ---------------------------------------------------------------------------
static uint8 s_bgm_cmd   = 0;
static int   s_bgm_state = 2;

static void bgm_handshake_step(void) {
    if (s_bgm_state == 0) {
        fprintf(stderr, "SPC BGM: sending start cmd $%02X\n", s_bgm_cmd);
        spc_write_port(s_spc, 0, 0, s_bgm_cmd);          // .start
        s_bgm_state = 1;
    } else if (s_bgm_state == 1) {
        if ((uint8)spc_read_port(s_spc, 0, 0) == s_bgm_cmd) {
            fprintf(stderr, "SPC BGM: driver echoed cmd $%02X\n", s_bgm_cmd);
            spc_write_port(s_spc, 0, 0, 0);              // .check: ack, clear
            s_bgm_state = 2;
        } else {
            spc_write_port(s_spc, 0, 0, s_bgm_cmd);      // re-send until seen
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

void SpcPlayer_Init(int sample_rate) {
    (void)sample_rate;  // we always run at SPC native 32000 Hz

    s_spc = spc_new();
    if (!s_spc) {
        fprintf(stderr, "SPC Player: failed to allocate SPC emulator\n");
        return;
    }

    s_filter = spc_filter_new();
    if (!s_filter) {
        fprintf(stderr, "SPC Player: failed to allocate SPC filter\n");
        return;
    }

    s_mutex = SDL_CreateMutex();
    if (!s_mutex) {
        fprintf(stderr, "SPC Player: failed to create mutex\n");
        return;
    }

    spc_reset(s_spc);
    spc_filter_clear(s_filter);
    spc_filter_set_gain(s_filter, spc_filter_gain_unit);
    spc_filter_set_bass(s_filter, spc_filter_bass_norm);

    s_pq_head = 0;
    s_pq_tail = 0;

    printf("SPC Player: initialized (snes_spc, native %d Hz)\n", spc_sample_rate);
}

void SpcPlayer_Shutdown(void) {
    if (s_filter) {
        spc_filter_delete(s_filter);
        s_filter = NULL;
    }
    if (s_spc) {
        spc_delete(s_spc);
        s_spc = NULL;
    }
    if (s_mutex) {
        SDL_DestroyMutex(s_mutex);
        s_mutex = NULL;
    }
}

// ---------------------------------------------------------------------------
// Sample generation  (called from SDL audio callback thread)
//
// Flushes any pending port writes at time 0 (start of this audio frame),
// then generates the requested number of sample pairs via spc_play().
// ---------------------------------------------------------------------------
void SpcPlayer_Generate(int16 *buffer, int num_samples) {
    if (!s_spc || !buffer) {
        memset(buffer, 0, num_samples * 2 * sizeof(int16));
        return;
    }

    SDL_LockMutex(s_mutex);

    // Flush queued port writes at time 0 (beginning of this frame)
    while (s_pq_tail != s_pq_head) {
        PortWrite *pw = &s_port_queue[s_pq_tail];
        spc_write_port(s_spc, 0, pw->port, pw->value);
        s_pq_tail = (s_pq_tail + 1) % PORT_QUEUE_SIZE;
    }

    // Run the BGM start handshake once per callback (~ once per frame)
    bgm_handshake_step();

    // Generate stereo samples at SPC native 32000 Hz
    int stereo_count = num_samples * 2;
    spc_err_t err = spc_play(s_spc, stereo_count, buffer);
    if (err) {
        memset(buffer, 0, stereo_count * sizeof(int16));
    }

    // Apply bass/gain filter
    if (s_filter) {
        spc_filter_run(s_filter, buffer, stereo_count);
    }

    // Debug: check if we're generating non-zero audio
    {
        static int s_dbg_count = 0;
        s_dbg_count++;
        if (s_dbg_count <= 10 || (s_dbg_count % 100 == 0 && s_dbg_count <= 1000)) {
            int32 peak = 0;
            for (int i = 0; i < stereo_count; i++) {
                int32 v = buffer[i] < 0 ? -buffer[i] : buffer[i];
                if (v > peak) peak = v;
            }
            fprintf(stderr, "Audio[%d]: %d samples, peak=%d\n",
                    s_dbg_count, num_samples, (int)peak);
        }
    }

    SDL_UnlockMutex(s_mutex);
}

// ---------------------------------------------------------------------------
// Port writes  (called from game thread)
//
// These are queued and applied at the start of the next audio callback.
// ---------------------------------------------------------------------------
void SpcPlayer_WritePort(int port, uint8 val) {
    if (!s_mutex || port < 0 || port >= spc_port_count) return;

    SDL_LockMutex(s_mutex);
    int next = (s_pq_head + 1) % PORT_QUEUE_SIZE;
    if (next != s_pq_tail) {
        s_port_queue[s_pq_head].port  = (uint8)port;
        s_port_queue[s_pq_head].value = val;
        s_pq_head = next;
    }
    SDL_UnlockMutex(s_mutex);
}

void SpcPlayer_SendCommand(uint8 cmd, uint8 param) {
    SpcPlayer_WritePort(0, cmd);
    SpcPlayer_WritePort(1, param);
}

// Queue a BGM start command (startbgm semantics: bgm_music + bgmcnt=0).
// The audio thread performs the port-0 write/echo handshake.
void SpcPlayer_StartBgm(uint8 cmd) {
    if (!s_mutex) return;
    SDL_LockMutex(s_mutex);
    s_bgm_cmd   = cmd;
    s_bgm_state = 0;
    SDL_UnlockMutex(s_mutex);
}

uint8 SpcPlayer_ReadPort(int port) {
    if (!s_spc || !s_mutex || port < 0 || port >= spc_port_count) return 0;

    SDL_LockMutex(s_mutex);
    // Read at time 0 — approximate but safe; the SPC driver updates ports
    // asynchronously as it runs in the audio callback.
    uint8 val = (uint8)spc_read_port(s_spc, 0, port);
    SDL_UnlockMutex(s_mutex);
    return val;
}

// ---------------------------------------------------------------------------
// Track loading  (called from game thread)
//
// Holds the mutex for the entire IPL boot upload so the audio callback
// doesn't try to generate samples from a half-initialized SPC.
// ---------------------------------------------------------------------------
void SpcPlayer_LoadTrack(uint8 track_id) {
    if (!s_mutex) return;

    SDL_LockMutex(s_mutex);

    SpcBoot_LoadTrack(track_id);

    // Clear the port queue — stale writes from before the reboot are invalid
    s_pq_head = 0;
    s_pq_tail = 0;

    // Cancel any in-flight BGM handshake; the caller queues the new track's
    // start command after the reboot.
    s_bgm_state = 2;

    SDL_UnlockMutex(s_mutex);
}

// Internal access for boot protocol (called under mutex from LoadTrack)
SNES_SPC *SpcPlayer_GetSPC(void) {
    return s_spc;
}
