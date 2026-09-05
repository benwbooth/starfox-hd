# SF1 presentation regression work — 2026-09-04

This ledger separates implemented repairs from reproduced symptoms and retail
evidence. Passing native rendering tests is not retail certification.

| Report | Evidence and current status |
| --- | --- |
| Low-rate floor dots | Fixed HD point interpolation. World grid cells retain identity across grid recentering; dust carries allocation generations; near-point second pixels have distinct identities. Fractional positions reach GPU quads without integer rounding. Source pixels, RNG and raster output are unchanged. Unit tests cover clipping/reordering, respawn and cell boundaries; a GPU readback test verifies the midpoint actually moves. |
| Staircase sky on camera banking | Fixed HD spatial interpolation across column and row offsets, including wrapped texture coordinates. Exact source-resolution strip rendering is retained. Temporal interpolation remains independent. |
| Tunnel faces flicker near the eye plane | Fixed an independently reproduced culling defect: a selector touching/crossing the eye plane made either side of a face visible. A homogeneous orientation determinant preserves the selected side without dividing by depth. The existing corridor depth bias is mathematically distance-independent; tests now pin that property. Full reported tunnel flicker is not yet closed. |
| First-hit pause, approximately five seconds | Confirmed synchronous first-use effect decoding under the audio mixer lock. Available effect clips now preload before runtime; a regression deletes the file after preload and verifies playback. Music is not bulk-preloaded (the local library is 3.1 GB). This eliminates one hitch path, not proof of the entire reported pause. |
| Slow launch sequence | Open. Runtime pacing still indexes the neutral-input timing recording. First 100 recorded intervals consume 684 display refreshes. Frame 186 contains an 87-refresh audio-transfer wait; frame 943 contains a 103-refresh interval at the neutral run's death/restart. Those waits are incorrectly applicable to arbitrary input solely by frame number. Do not replace these with guessed timing or bless a native fixture; recover context-dependent timing and validate distinct input tapes. |
| Gray tower/base insignia blinks | Open. `mybase_0` insignia stays visible in GPU samples at four depths and five yaw angles. That rules out a completely missing texture, not intermittent occlusion during gameplay. The separate rotating `tower_2` has no logo texture or color animation. Capture the actual affected object/lifetime before altering its material. |
| Arch acknowledgment sound | Source reviewed: `hard_Istrat` creates the solid arch; its three collision boxes can trigger impact audio. Paired `skillfly` markers change a skill counter but have no sound trigger. Separate healing gates explicitly trigger sounds 15/16. No invented arch sound added. |
| General animation smoothing | Object/camera transforms already interpolate. Point fields now do too. Shape-frame switches, palette animation, spawning, destruction and dialogue are discrete; blending all of them indiscriminately would invent invalid geometry/events. Compatible vertex animation needs an explicit topology contract and separate tests. |
| Missing intro siren and emergency voice | Fixed a statically proven omission: the audio layer explicitly skipped BGS `bgm 10` and booted the later Corneria bank. It now receives the live background id, starts SND_10/cue 16 for the scramble, and switches once to SND_11/cue 3 on ground background 4. Blink backgrounds do not restart music; a ground checkpoint restart does not replay the announcement. The existing intro audio asset is present. Bank/cue tests pass; live acoustic presentation has not been rechecked. |
| Missing rectangular tunnel outline | OP_0 already contains the authored wireframe and is spawned in the proper map order. Fixed inverted HD depth priority: the outline now wins its coplanar OP_1 backing, matching the source equal-depth painter ordering. A test pins this priority independently of draw-list order. Full moving-scene appearance remains to be confirmed. |
| Missing blue smoke when wingmen launch | Fixed two renderer defects. The authored boost billboard was completely discarded by ordinary polygon back-face culling; a GPU regression failed before the fix and passes for both texture frames afterward. Its HD size also omitted the source's doubled extent and used the header extent instead of actual mesh width as a denominator. Tests pin world widths 40, 38 and 30 for the relevant signed adjustments. Strict source sprites no longer fall through into a duplicate HD quad. |

## Static Corneria launch audit

This pass follows the call/data flow from map commands through strategies to
rendering and audio consumers. Finding a spawn in Rust is insufficient: the
boost object existed but its renderer discarded it. The scope is the Corneria
launch corridor, not all title/attract sequences or the entire game.

Paths below are relative to `reference/ultrastarfox/SF/` for assembly and
`rust/` for Rust.

| Contract reviewed | Source | Rust and result |
| --- | --- | --- |
| Tunnel pairs, wingman positions/delays, eight corridor extensions, strategy-complete exit | `MAPS/MAP1_1A.ASM`; `INC/MAPMACS.INC` | `sf-map/src/levels/level1_1.rs::append_map1_1a_submap`: checked object order, waits, loop and exit. `mapobjnomem` is a source macro alias, not missing memory behavior. |
| Wingman launch and lifetime | `STRAT/GISTRATS.ASM::shipintro_Istrat/shipintro_strat` | `sf-strat/src/player.rs`: checked signed delay, boost allocation, speed increase, shadow/float/removal flags and lifetime. No additional strategy discrepancy established. |
| Player opening, camera handoff and boost | `STRAT/PISTRATS.ASM::playeropening_Istrat/playeropening_strat`; `STRAT/GISTRATS.ASM::viewopening` | `sf-strat/src/player.rs`, `sf-game/src/camera.rs`: checked control flags, completion latch, 70-tick wait, boost and camera position/target handling. No new discrepancy established. This does not certify wall-clock pacing. |
| Intro soundtrack and exit transition | `ASM/BGS.ASM::bg_1_1i_1/bg_1_1c_1`; `ASM/SOUND.ASM::sndtbl` | `sf-app/src/audio.rs`, `sf-audio/src/sound.rs`: repaired missing intro bank and checked the application-to-native-player path for overriding commands. |
| Outline and solid backing ordering | `SHAPES/SHAPES.ASM::op_0/op_1`; `MARIO/MDRAWLIS.MC` | `sf-render/src/draw_list.rs`: repaired coplanar priority. |
| Boost visibility, texture frames and signed square size | `SHAPES/USHAPES.ASM::boostshape`; `STRAT/GSTRATS.ASM::boost_Istrat/boost_strat`; `MARIO/MDRAWLIS.MC`, `MARIO/MDSPRITE.MC::mssprite` | `sf-render/src/draw_list.rs`, `shapes_gl.rs`: repaired billboard culling and sizing. The source uses twice the header extent plus the signed adjustment shifted by the shape's coordinate scale. |

Repeatable workflow: identify an assembly entry point and its reachable
consumers; enumerate effects and branch conditions; compare typed Rust state
and operations; encode each demonstrated discrepancy as a focused regression;
repair it; retain ambiguous timing, rendering or source-version questions as
open evidence gaps. Use independent oracle samples for those gaps, not a full
playthrough after every statically provable correction. Static review plus
self-consistency tests alone cannot certify complete retail equivalence.

## Unattended HD capture

`sf1_hd_presentation_probe` is a native diagnostic, not an independent oracle.
It drives legal controller input, preserves the completed-scene queue and
camera/background/point histories, and samples five render phases per requested
gameplay interval. It uses the shipping default of disabled shadows and smooth
polygon shading at 1280 by 720. It does not read custom `starfox.ini` settings.

```sh
SF1_HD_PRESENTATION_FIRST_FRAME=130 \
SF1_HD_PRESENTATION_LAST_FRAME=134 \
SF1_HD_PRESENTATION_OUT_DIR=/tmp/sf1-hd-corridor-example \
nix develop --command cargo run --manifest-path rust/Cargo.toml \
  -p sf-oracle --example sf1_hd_presentation_probe
```

Default gameplay input is neutral. Set `SF1_HD_PRESENTATION_ATTACK=1` to use
the existing deterministic attack tape. Output includes PPMs and tab-separated
scene/camera/view-mode metadata. An incomplete requested range fails instead
of reporting successful capture. The probe deliberately does not run audio.

## Verification

- After the static intro repairs, `cargo test -p sf-render -p sf-audio`
  passed 258 tests, including five GPU/runtime tests, and `cargo build
  -p sf-app` passed. Log: `/tmp/sf1-static-intro-tests-20260904.log`.
  The native architecture check and `git diff --check` also passed.
- Before the final culling change, `cargo test -p sf-game -p sf-render
  -p sf-audio` passed 453 tests, including four GPU/runtime tests; app build
  passed. Log: `/tmp/sf1-presentation-package-tests-20260904.log`.
- Native capture ranges 120–123 and 130–134 produced exactly 20 and 25
  phase images. These inspect HD continuity only, not retail parity.
- Prior independent semantic/timing discrepancies in `FINISHING_AUDIT.md`
  remain open. No oracle expected values were regenerated in this work.

Source paths for arch review: `reference/ultrastarfox/SF/MAPS/1-1.ASM`,
`STRAT/GSTRATS.ASM` (`hard_Istrat`), `STRAT/DSTRATS.ASM` (`skillfly` and
`gate_strat`), `STRAT/GA2STRAT.ASM` (`gate3_strat`), and `COLBOXES.ASM`.
