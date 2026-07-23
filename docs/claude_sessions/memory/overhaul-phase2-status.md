---
name: overhaul-phase2-status
description: "Post-queue overhaul (June 2026): phase-1 queue drained but game wasn't playable; four-lane fix effort and root causes"
metadata: 
  node_type: memory
  type: project
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Phase-1 port queue drained (154 done) but the game only showed a rotating Arwing. Deep diagnosis (2026-07-01) found four localized root causes, fixed in four parallel lanes:

1. **Colors**: palette data in `src/renderer/shapes.c` is byte-exact/correct; defect was missing per-face lighting (COLLITE shade 0-9 from normal·light via `light_data.h` shadesG_M tables) + COLDEPTH locked to night1 bank. Proven reference impl: `src/viewer/shape_catalog_asm.c:1203-1224`.
2. **Audio**: pipeline ~90% wired (SDL→snes_spc→port protocol all connected); root cause was `spc_boot.c` fabricated-snapshot boot with zeroed DSP regs (MVOL=0) instead of real IPL handshake (`ipl_wait_ready`/`ipl_wait_echo` existed as dead code). Sound data = byte-exact copies of `reference/ultrastarfox/SF/SND/*.BIN`, format `[len][dest][data...]` blocks, exec $0400. Port protocol in `SOUND.ASM:128-281`, SFX handshake `IRQ.ASM:1612-1632`.
3. **2D pipeline**: renderer had zero 2D passes; `setbg`/fades/planet-select computed state nobody drew. Assets `data/gfx_title.bin`, `palettes.bin` were on disk unreferenced. Font/sprite GL plumbing in `hud.c`/`font.c`/`sprites.c` works and is the pattern to reuse.
4. **Progression**: gameplay core + Arwing flight fully functional, but `g_levelfinished` had no consumer, death had no respawn/game-over, HUD setters never fed. `GAME_STATE_CONTINUE`/`ENDING` were unreachable.

Input: Enter=START (Space added during overhaul). Game logic is fixed 20Hz tick with render interpolation in main.c. Resolution/fullscreen/MSAA/widescreen/TargetFPS already in starfox.ini/config.c.

Parallel agent lanes used separate build dirs (build-colors, build-audio, build-2d, build-prog) to avoid clashes; merge back to `build/` after.

**Status end of 2026-07-01 session:** C oracle committed at `b5fe8db` (playable: title/music/planet-select/Corneria with intro, bosses 1/2/7/8/A/F/Seamon/G, horizon-locked backgrounds, HUD+portraits). RIIR wave 1 committed (`60000a1`, `d4a5a34`): sf-render data + sf-map phase 1 (level 1_1/title/planet byte-identical) + sf-path catalog byte-identical (8437 bytes/359 offsets); 22 workspace tests green. Test harness: SF_AUTOPLAY / SF_DUMP_PPM / SF_STATE_DUMP / SF_AUDIO_WAV; difftest at rust/sf-difftest. Generator: tools/gen_path_catalog_rust.py. Wave 2 queued (task list): remaining levels, interp, game core w/ difftest, SDL3 app shell. Known cosmetic: FX102 chicken_spr COLTEXT renders debug color on mybase_0.

User goal (stated 2026-07-01): decompiled PC port of Star Fox AND Star Fox 2, then higher framerates + resolution options. SF2: final-release ROM "Star Fox 2 (USA, Europe).sfc" is in the repo root (confirmed 2026-07-02); no source/disassembly in reference/ yet — recon phase started (docs/SF2_RECON.md when done); recommended to user: acquire an SF2 source reference equivalent to ultrastarfox for transcription-style porting. SF2 port phase follows the SF1 RIIR finish. [[starfox2-scope]]
