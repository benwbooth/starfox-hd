// SPC700 IPL Boot Protocol Implementation
//
// Uploads the Star Fox sound driver and BGM data into the SPC700 emulator via
// the authentic SNES IPL ROM handshake protocol (SOUND.ASM sbootapu), parsing
// the self-describing sub-block BIN format used by Star Fox's sound data
// files: repeated [len:2 LE][dest:2 LE][data...] blocks terminated by
// [00 00][exec:2 LE].  The per-file exec word is ignored; after all files of
// a track are uploaded, execution is transferred to the driver entry point
// ($0400), exactly like sbootapu does.

#include "spc_boot.h"
#include "spc_player.h"
#include <spc.h>
#include <SDL2/SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ---------------------------------------------------------------------------
// Standard 64-byte SNES IPL ROM
// ---------------------------------------------------------------------------
static const uint8 s_ipl_rom[64] = {
    0xCD,0xEF,0xBD,0xE8,0x00,0xC6,0x1D,0xD0,0xFC,0x8F,0xAA,0xF4,0x8F,0xBB,0xF5,0x78,
    0xCC,0xF4,0xD0,0xFB,0x2F,0x19,0xEB,0xF4,0xD0,0xFC,0x7E,0xF4,0xD0,0x0B,0xE4,0xF5,
    0xCB,0xF4,0xD7,0x00,0xFC,0xD0,0xF3,0xAB,0x01,0x10,0xEF,0x7E,0xF4,0x10,0xEB,0xBA,
    0xF6,0xDA,0x00,0xBA,0xF4,0xC4,0xF4,0xDD,0x5D,0xD0,0xDB,0x1F,0x00,0x00,0xC0,0xFF
};

// ---------------------------------------------------------------------------
// Track-to-file mapping table (from SOUND.ASM sndtbl)
// Each track lists its BIN files; NULL terminates the list.
// SGSOUND0.BIN (the driver) is always loaded first, separately.
// ---------------------------------------------------------------------------
// Label -> file map (BANK/INCBINS.ASM incsnd + BANK8/9/10 + SHBANKS.ASM):
//   sound0=SGSOUND0  sound1=SGSOUND1  orchestra=SGSOUND2  band=SGSOUND3
//   sound4=SGSOUND4  sound5=SGSOUND5  sound6=SGSOUND6  titlebgm=SGSOUND7
//   bgmendseq=SGSOUND8  bgmcontinue=SGSOUND9  bgmstaff=SGSOUNDA
//   corneria=SGBGMA  meteor_base=SGBGMB  titania=SGBGMC  fortuna=SGBGMD
//   macbeth=SGBGME  asteroid_field=SGBGMF  space_armada=SGBGMG
//   venom_base_2=SGBGMH  sector_x_z=SGBGMI  sector_y=SGBGMJ
//   venom_base_1_3=SGBGMK  black_holebgm=SGBGML  bgmintro=SGBGMM
//   ops_bgm=SGBGMN  bgmtraining=SGBGMO  out_of_this_dimension=SGBGMP
//   boss_corneria=SGBGM1  boss_fortuna=SGBGM2  boss_macbeth=SGBGM3
//   boss_titania=SGBGM4  boss_meteor=SGBGM5  boss_space_armada=SGBGM6
//   boss_asteroid=SGBGM7  boss_venom_base_2=SGBGM8  boss_venom_base_1=SGBGM9
//   venom_andross=SGBGM10  game_overbgm=SGBGM11
static const char *s_track_files[][8] = {
    [SND_INIT]     = { "SGSOUND0.BIN", NULL },
    [SND_INTRO]    = { "SGSOUND1.BIN", "SGSOUND2.BIN", "SGBGMM.BIN", NULL },
    [SND_TITLE]    = { "SGSOUND7.BIN", NULL },
    [SND_OPS]      = { "SGSOUND1.BIN", "SGBGMN.BIN", NULL },
    [SND_TRAINING] = { "SGSOUND3.BIN", "SGBGMO.BIN", NULL },
    [SND_MAP]      = { "SGSOUND2.BIN", "SGSOUND6.BIN", NULL },
    [SND_CONTINUE] = { "SGSOUND9.BIN", NULL },
    [SND_BHOLE]    = { "SGBGML.BIN", NULL },
    [SND_10]       = { "SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN", NULL },
    [SND_11]       = { "SGSOUND1.BIN", "SGSOUND3.BIN", "SGBGMA.BIN", "SGBGM1.BIN", NULL },
    [SND_12]       = { "SGBGMF.BIN", "SGBGM7.BIN", NULL },
    [SND_13]       = { "SGBGMG.BIN", "SGBGM6.BIN", NULL },
    [SND_13B]      = { "SGBGMG.BIN", "SGBGM6.BIN", NULL },
    [SND_14]       = { "SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMB.BIN", "SGBGM5.BIN", NULL },
    [SND_15]       = { "SGBGMF.BIN", "SGBGM8.BIN", NULL },
    [SND_16]       = { "SGBGMK.BIN", "SGBGM8.BIN", "SGBGM10.BIN", NULL },
    [SND_20]       = { "SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN", NULL },
    [SND_21]       = { "SGSOUND1.BIN", "SGSOUND3.BIN", NULL },
    [SND_22]       = { "SGBGMI.BIN", "SGBGM7.BIN", NULL },
    [SND_23]       = { "SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMC.BIN", "SGBGM4.BIN", NULL },
    [SND_24]       = { "SGBGMJ.BIN", "SGBGM7.BIN", NULL },
    [SND_25]       = { "SGBGMF.BIN", "SGBGM8.BIN", NULL },
    [SND_26]       = { "SGBGMH.BIN", "SGBGM8.BIN", "SGBGM10.BIN", NULL },
    [SND_30]       = { "SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN", NULL },
    [SND_31]       = { "SGSOUND1.BIN", "SGSOUND3.BIN", NULL },
    [SND_32]       = { "SGBGMF.BIN", "SGBGM7.BIN", NULL },
    [SND_33]       = { "SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMD.BIN", "SGBGM2.BIN", NULL },
    [SND_34]       = { "SGBGMI.BIN", "SGBGM6.BIN", NULL },
    [SND_35]       = { "SGSOUND3.BIN", "SGBGME.BIN", "SGBGM3.BIN", NULL },
    [SND_36]       = { "SGBGMF.BIN", "SGBGM9.BIN", NULL },
    [SND_37]       = { "SGBGMK.BIN", "SGBGM9.BIN", "SGBGM10.BIN", NULL },
    [SND_ENDSEQ]   = { "SGSOUND8.BIN", NULL },
    [SND_STAFF]    = { "SGSOUNDA.BIN", NULL },
    [SND_GAMEOVER] = { "SGBGM11.BIN", NULL },
    [SND_SPECIAL]  = { "SGSOUND3.BIN", "SGBGMP.BIN", NULL },
};

// ---------------------------------------------------------------------------
// Per-track BGM start command (2nd argument of snd_data in SOUND.ASM sndtbl).
// After sbootapu finishes, the game's startmus loop (IRQ.ASM) writes this
// value to APU port 0 to actually start the music.
// ---------------------------------------------------------------------------
static const uint8 s_track_bgm_cmd[SND_TRACK_COUNT] = {
    [SND_INIT]     = 0x00,
    [SND_INTRO]    = 0x12,
    [SND_TITLE]    = 0x12,
    [SND_OPS]      = 0x12,
    [SND_TRAINING] = 0x03,
    [SND_MAP]      = 0x01,
    [SND_CONTINUE] = 0x0A,
    [SND_BHOLE]    = 0x03,
    [SND_10]       = 0x10,   // snd_data 10,16 (decimal)
    [SND_11]       = 0x03,
    [SND_12]       = 0x03,
    [SND_13]       = 0x09,
    [SND_13B]      = 0x03,
    [SND_14]       = 0x03,
    [SND_15]       = 0x03,
    [SND_16]       = 0x03,
    [SND_20]       = 0x10,
    [SND_21]       = 0x03,
    [SND_22]       = 0x03,
    [SND_23]       = 0x03,
    [SND_24]       = 0x03,
    [SND_25]       = 0x03,
    [SND_26]       = 0x03,
    [SND_30]       = 0x10,
    [SND_31]       = 0x03,
    [SND_32]       = 0x03,
    [SND_33]       = 0x03,
    [SND_34]       = 0x03,
    [SND_35]       = 0x03,
    [SND_36]       = 0x03,
    [SND_37]       = 0x03,
    [SND_ENDSEQ]   = 0x03,
    [SND_STAFF]    = 0x12,
    [SND_GAMEOVER] = 0x0C,
    [SND_SPECIAL]  = 0x03,
};

uint8 SpcBoot_TrackCommand(uint8 track_id) {
    if (track_id < SND_TRACK_COUNT)
        return s_track_bgm_cmd[track_id];
    return track_id;  // out-of-table values are raw driver commands
}

// ---------------------------------------------------------------------------
// Common-bank prerequisites.
//
// The original never clears APU RAM between boots: sbootapu re-enters the IPL
// with the $FF command and uploads only the listed files, so gameplay rows
// like `snd_data 12,3,asteroid_field,boss_asteroid` (a few KB of sequence
// data at $E000+) rely on the common sample banks staying resident:
//   SGSOUND1 (sound1): sample dir $3C00, BRR data $4000-$B360 (SFX + base)
//   SGSOUND2 (orchestra): dir $3C60, data $B360-$DCF0, insts $3D00,
//                         song table + fanfares $F342-$FDE2
// (sound1 comes from the ops/intro boot, orchestra is refreshed by the map
// screen's `bgm map` before every level.)  Because we power-cycle the SPC on
// every load, gameplay tracks must re-upload those banks first.  Rows that
// carry their own bank (band = SGSOUND3 overlays orchestra's range, sound5
// overlays both) upload it afterwards and win, matching the original's final
// RAM state.  UI tracks (title/intro/ops/endseq/staff) are self-contained.
// ---------------------------------------------------------------------------
static bool track_needs_common_banks(uint8 track_id) {
    switch (track_id) {
    case SND_INIT:
    case SND_INTRO:
    case SND_TITLE:
    case SND_OPS:
    case SND_ENDSEQ:
    case SND_STAFF:
        return false;
    default:
        return true;  // all gameplay/map/continue/gameover/bhole rows
    }
}

// ---------------------------------------------------------------------------
// IPL handshake timing
// ---------------------------------------------------------------------------
#define IPL_STEP       64       // SPC clocks per polling step (~2 sample pairs)
#define IPL_TIMEOUT    50000    // max polling steps waiting for a port echo

// Dummy output buffer for SPC emulation during boot upload
#define BOOT_BUF_SIZE  4096
static int16 s_boot_buf[BOOT_BUF_SIZE];

// ---------------------------------------------------------------------------
// SPC driver entry point
// ---------------------------------------------------------------------------
#define SPC_DRIVER_ENTRY 0x0400

// ---------------------------------------------------------------------------
// Asset directory handling.  Sound data lives at <asset_dir>/snd/.  We first
// resolve the asset dir relative to the executable's directory, then fall
// back to the current working directory.
// ---------------------------------------------------------------------------
static char s_asset_dir[512] = "data";

void SpcBoot_SetAssetDir(const char *dir) {
    if (dir && dir[0]) {
        snprintf(s_asset_dir, sizeof(s_asset_dir), "%s", dir);
    }
}

// ---------------------------------------------------------------------------
// Read a BIN file from the sound data directory into a malloc'd buffer.
// Returns NULL on failure, sets *out_size.
// ---------------------------------------------------------------------------
static uint8 *load_bin_file(const char *filename, int *out_size) {
    char path[1024];
    FILE *f = NULL;

    *out_size = 0;

    // 1) Executable-relative (skip if asset dir is absolute)
    if (s_asset_dir[0] != '/') {
        char *base = SDL_GetBasePath();
        if (base) {
            snprintf(path, sizeof(path), "%s%s/snd/%s", base, s_asset_dir, filename);
            f = fopen(path, "rb");
            SDL_free(base);
        }
    }

    // 2) Asset dir as-is (absolute path or relative to cwd)
    if (!f) {
        snprintf(path, sizeof(path), "%s/snd/%s", s_asset_dir, filename);
        f = fopen(path, "rb");
    }

    if (!f) {
        fprintf(stderr, "SPC Boot: cannot open %s (asset dir '%s', exe-relative and cwd tried)\n",
                filename, s_asset_dir);
        return NULL;
    }

    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    if (sz <= 0) {
        fclose(f);
        return NULL;
    }

    uint8 *buf = (uint8 *)malloc((size_t)sz);
    if (!buf) {
        fclose(f);
        return NULL;
    }

    size_t rd = fread(buf, 1, (size_t)sz, f);
    fclose(f);

    *out_size = (int)rd;
    return buf;
}

// ---------------------------------------------------------------------------
// Advance the SPC emulator by a small time step, keeping the output buffer
// rotating so it never overflows.
// ---------------------------------------------------------------------------
static void spc_advance(SNES_SPC *spc, spc_time_t clocks) {
    spc_end_frame(spc, clocks);
    // After end_frame the internal time resets to 0.  Re-arm the output
    // buffer so subsequent calls don't overflow.
    if (spc_sample_count(spc) >= BOOT_BUF_SIZE - 64) {
        spc_set_output(spc, s_boot_buf, BOOT_BUF_SIZE);
    }
}

// ---------------------------------------------------------------------------
// Wait for the IPL ROM ready signal ($AA on port0, $BB on port1).
// Returns true on success.
// ---------------------------------------------------------------------------
static bool ipl_wait_ready(SNES_SPC *spc) {
    for (int i = 0; i < IPL_TIMEOUT; i++) {
        int p0 = spc_read_port(spc, 0, 0);
        int p1 = spc_read_port(spc, 0, 1);
        if (p0 == 0xAA && p1 == 0xBB)
            return true;
        spc_advance(spc, IPL_STEP);
    }
    fprintf(stderr, "SPC Boot: timeout waiting for IPL ready\n");
    return false;
}

// ---------------------------------------------------------------------------
// Wait for port 0 to echo a specific value.
// Returns true on success.
// ---------------------------------------------------------------------------
static bool ipl_wait_echo(SNES_SPC *spc, uint8 val) {
    for (int i = 0; i < IPL_TIMEOUT; i++) {
        int p0 = spc_read_port(spc, 0, 0);
        if ((p0 & 0xFF) == val)
            return true;
        spc_advance(spc, IPL_STEP);
    }
    fprintf(stderr, "SPC Boot: timeout waiting for echo $%02X\n", val);
    return false;
}

// ---------------------------------------------------------------------------
// Upload one data block via the IPL port protocol (SOUND.ASM sboot_entry4 /
// sboot_repeat / sboot_loop).
//
// *cmd holds the "block command" value written to port 0 to announce the
// block ($CC for the very first block after IPL ready; afterwards derived
// from the last byte index).  It is updated for the next block on return.
// ---------------------------------------------------------------------------
static bool ipl_upload_block(SNES_SPC *spc, uint16 dest,
                             const uint8 *data, int len, uint8 *cmd) {
    // Announce the block: dest -> ports 2/3, non-zero -> port 1 (data
    // follows), block command -> port 0.  The IPL echoes the command.
    spc_write_port(spc, 0, 2, dest & 0xFF);
    spc_write_port(spc, 0, 3, (dest >> 8) & 0xFF);
    spc_write_port(spc, 0, 1, (len >= 1) ? 1 : 0);
    spc_write_port(spc, 0, 0, *cmd);
    if (!ipl_wait_echo(spc, *cmd)) {
        fprintf(stderr, "SPC Boot: no ack for block header ($%04X, %d bytes)\n", dest, len);
        return false;
    }

    // Send bytes: data -> port 1, index -> port 0; wait for the IPL to echo
    // the index back after storing the byte.  Index restarts at 0 each block
    // and wraps mod 256 (the IPL tracks the carry into the dest high byte).
    uint8 idx = 0;
    for (int i = 0; i < len; i++, idx++) {
        spc_write_port(spc, 0, 1, data[i]);
        spc_write_port(spc, 0, 0, idx);
        if (!ipl_wait_echo(spc, idx)) {
            fprintf(stderr, "SPC Boot: stalled at byte %d/%d of block $%04X\n", i, len, dest);
            return false;
        }
    }

    // Next block command = last index + 4, skipping 0 (sboot_zero).
    uint8 next = (uint8)(idx + 3);   // idx == last_index + 1
    if (next == 0) next = 4;
    *cmd = next;
    return true;
}

// ---------------------------------------------------------------------------
// Upload all sub-blocks of one BIN file.  Stops at the [00 00] terminator
// (the trailing per-file exec word is ignored, exactly like sboot_entry3).
// ---------------------------------------------------------------------------
static bool ipl_upload_file(SNES_SPC *spc, const uint8 *data, int size, uint8 *cmd) {
    int pos = 0;

    while (pos + 4 <= size) {
        uint16 count = (uint16)(data[pos] | (data[pos + 1] << 8));
        uint16 dest  = (uint16)(data[pos + 2] | (data[pos + 3] << 8));
        pos += 4;

        if (count == 0)
            break;  // terminator; 'dest' is the ignored per-file exec addr

        if (pos + count > size) {
            fprintf(stderr, "SPC Boot: sub-block overflows file (%d bytes at $%04X, %d remain)\n",
                    count, dest, size - pos);
            return false;
        }

        if (!ipl_upload_block(spc, dest, &data[pos], count, cmd))
            return false;
        pos += count;
    }

    return true;
}

// ---------------------------------------------------------------------------
// Transfer control to an SPC address (SOUND.ASM sboot_entry3 .end):
// exec addr -> ports 2/3, zero -> port 1 (jump, not data), command -> port 0.
// The IPL echoes the command and jumps.  Afterwards ports 1-3 are zeroed
// like the original (stz apu_port1/2/3).
// ---------------------------------------------------------------------------
static bool ipl_start_exec(SNES_SPC *spc, uint16 addr, uint8 cmd) {
    spc_write_port(spc, 0, 2, addr & 0xFF);
    spc_write_port(spc, 0, 3, (addr >> 8) & 0xFF);
    spc_write_port(spc, 0, 1, 0);
    spc_write_port(spc, 0, 0, cmd);
    if (!ipl_wait_echo(spc, cmd)) {
        fprintf(stderr, "SPC Boot: no ack for exec at $%04X\n", addr);
        return false;
    }

    spc_write_port(spc, 0, 1, 0);
    spc_write_port(spc, 0, 2, 0);
    spc_write_port(spc, 0, 3, 0);
    return true;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

void SpcBoot_Init(void) {
    SNES_SPC *spc = SpcPlayer_GetSPC();
    if (!spc) return;
    spc_init_rom(spc, s_ipl_rom);
    printf("SPC Boot: IPL ROM installed\n");
}

bool SpcBoot_LoadTrack(uint8 track_id) {
    SNES_SPC *spc = SpcPlayer_GetSPC();
    if (!spc) {
        fprintf(stderr, "SPC Boot: no SPC emulator available\n");
        return false;
    }

    if (track_id >= SND_TRACK_COUNT) {
        fprintf(stderr, "SPC Boot: invalid track ID %d\n", track_id);
        return false;
    }

    printf("SPC Boot: loading track %d\n", track_id);

    // -----------------------------------------------------------------------
    // 1. Power-cycle the SPC and let the IPL ROM come up.  (The original
    //    instead pokes $FF at the resident driver to re-enter the IPL; a
    //    full reset reaches the same state since we re-upload the driver.)
    // -----------------------------------------------------------------------
    spc_init_rom(spc, s_ipl_rom);
    spc_reset(spc);
    spc_set_output(spc, s_boot_buf, BOOT_BUF_SIZE);

    if (!ipl_wait_ready(spc))
        return false;

    // First block is announced with the magic $CC kick (sboot_initial).
    uint8 cmd = 0xCC;

    // -----------------------------------------------------------------------
    // 2. Build the upload list: driver, resident common banks (if the track
    //    needs them), then the track's own files.  Duplicates are skipped so
    //    rows that already list sound1/band don't upload twice; later files
    //    intentionally overwrite earlier ones (band over orchestra, etc.).
    // -----------------------------------------------------------------------
    const char *upload_list[12];
    int upload_count = 0;

    upload_list[upload_count++] = "SGSOUND0.BIN";
    if (track_needs_common_banks(track_id)) {
        upload_list[upload_count++] = "SGSOUND1.BIN";
        upload_list[upload_count++] = "SGSOUND2.BIN";
    }
    {
        const char **files = s_track_files[track_id];
        for (int i = 0; i < 8 && files[i] != NULL; i++) {
            bool dup = false;
            for (int j = 0; j < upload_count; j++) {
                if (strcmp(files[i], upload_list[j]) == 0) {
                    dup = true;
                    break;
                }
            }
            if (!dup && upload_count < (int)(sizeof(upload_list) / sizeof(upload_list[0])))
                upload_list[upload_count++] = files[i];
        }
    }

    for (int i = 0; i < upload_count; i++) {
        int fsize = 0;
        uint8 *fdata = load_bin_file(upload_list[i], &fsize);
        if (!fdata) {
            if (i == 0) {
                fprintf(stderr, "SPC Boot: cannot load driver SGSOUND0.BIN — no audio\n");
                return false;
            }
            fprintf(stderr, "SPC Boot: skipping missing file %s\n", upload_list[i]);
            continue;
        }
        printf("  uploading %s (%d bytes)\n", upload_list[i], fsize);

        bool ok = ipl_upload_file(spc, fdata, fsize, &cmd);
        free(fdata);
        if (!ok) return false;
    }

    // -----------------------------------------------------------------------
    // 3. Jump to the driver entry point ($0400).
    // -----------------------------------------------------------------------
    if (!ipl_start_exec(spc, SPC_DRIVER_ENTRY, cmd))
        return false;

    // Let the driver run its init before any BGM/SFX commands are written.
    // On real hardware at least a frame passes before startmus (IRQ) writes
    // port 0; the driver samples the ports as its idle baseline during init,
    // so a command written too early is never seen as a change.  ~130 ms.
    for (int i = 0; i < 64; i++)
        spc_advance(spc, 2048);

    printf("SPC Boot: track %d uploaded, driver started at $%04X (bgm cmd $%02X)\n",
           track_id, SPC_DRIVER_ENTRY, s_track_bgm_cmd[track_id]);
    return true;
}
