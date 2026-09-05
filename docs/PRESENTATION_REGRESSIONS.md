# SF1 presentation regression work — 2026-09-04

This ledger separates implemented repairs from reproduced symptoms and retail
evidence. Passing native rendering tests is not retail certification.

| Report | Evidence and current status |
| --- | --- |
| Low-rate floor dots | Fixed HD point interpolation. World grid cells retain identity across grid recentering; dust carries allocation generations; near-point second pixels have distinct identities. Fractional positions reach GPU quads without integer rounding. Source pixels, RNG and raster output are unchanged. Unit tests cover clipping/reordering, respawn and cell boundaries; a GPU readback test verifies the midpoint actually moves. |
| Staircase sky on camera banking | Fixed HD spatial interpolation across column and row offsets, including wrapped texture coordinates. Exact source-resolution strip rendering is retained. Temporal interpolation remains independent. |
| Tunnel faces flicker near the eye plane | Fixed an independently reproduced culling defect: a selector touching/crossing the eye plane made either side of a face visible. A homogeneous orientation determinant preserves the selected side without dividing by depth. The existing corridor depth bias is mathematically distance-independent; tests now pin that property. Full reported tunnel flicker is not yet closed. |
| First-hit pause, approximately five seconds | Confirmed synchronous first-use effect decoding under the audio mixer lock. Available effect clips now preload before runtime; a regression deletes the file after preload and verifies playback. Music is not bulk-preloaded (the local library is 3.1 GB). This eliminates one hitch path, not proof of the entire reported pause. |
| Slow launch sequence / repeatable mid-level pause | Partially repaired. The 103-refresh neutral-recording death/restart interval at frame 943 is no longer applied to an alive player; live death/restart flags gate that wait. Both the public shell boundary and timing helper are tested. The original measurement arrays are unchanged. Frame 186 still includes the 87-refresh audio-transfer wait, and ordinary pacing still comes from one neutral-input recording. A native neutral trace confirms the death flag at scene 943 and completed restart at 944. This fixes an erroneous approximately 1.7-second pause, not proof that the entire reported five-second stall is gone. Fully context-dependent pacing remains open. |
| Gray tower/base insignia blinks | Reproduced and repaired for the level building `bu_7` (shape 67). `SHAPES4.ASM::bu_7_f1` draws logo face 8 inside coplanar backing face 7. Equal-depth comparison alone is insufficient because the smaller decal and larger wall have different triangulations: the pre-fix GPU test lost 2,963 logo pixels at depth 800/yaw -13. The renderer now recognizes an authored coplanar decal contained by the immediately preceding wall and applies one small normalized-depth layer. Transparent texels retain the backing; exploded geometry and source rasterization are unaffected. A 45-pose asset-driven GPU test covers this building and both `mybase_0` decals, comparing combined rendering to separately rendered logo and wall layers. Earlier `mybase_0`-only tests also passed without the repair and were insufficient to diagnose the level building. |
| Arch acknowledgment sound | Source reviewed: `hard_Istrat` creates the solid arch; its three collision boxes can trigger impact audio. Paired `skillfly` markers change a skill counter but have no sound trigger. Separate healing gates explicitly trigger sounds 15/16. No invented arch sound added. |
| General animation smoothing | Object/camera transforms already interpolate. Point fields now do too. Shape-frame switches, palette animation, spawning, destruction and dialogue are discrete; blending all of them indiscriminately would invent invalid geometry/events. Compatible vertex animation needs an explicit topology contract and separate tests. |
| Missing intro siren and emergency voice | Fixed a statically proven omission: the audio layer explicitly skipped BGS `bgm 10` and booted the later Corneria bank. It now receives the live background id, starts SND_10/cue 16 for the scramble, and switches once to SND_11/cue 3 on ground background 4. Blink backgrounds do not restart music; a ground checkpoint restart does not replay the announcement. The existing intro audio asset is present. Bank/cue tests pass; live acoustic presentation has not been rechecked. |
| Missing rectangular tunnel outline | OP_0 already contains the authored wireframe and is spawned in the proper map order. Fixed inverted HD depth priority: the outline now wins its coplanar OP_1 backing, matching the source equal-depth painter ordering. A test pins this priority independently of draw-list order. Full moving-scene appearance remains to be confirmed. |
| Missing blue smoke when wingmen launch | Fixed two renderer defects. The authored boost billboard was completely discarded by ordinary polygon back-face culling; a GPU regression failed before the fix and passes for both texture frames afterward. Its HD size also omitted the source's doubled extent and used the header extent instead of actual mesh width as a denominator. Tests pin world widths 40, 38 and 30 for the relevant signed adjustments. Strict source sprites no longer fall through into a duplicate HD quad. |
| Launch blast squashed and repeated horizontally | Follow-up testing exposed a further bitmap-mapping defect: the size byte was still consumed as polygon texture scrolling, wrapping across the quad. SF1 HD sprites now take a dedicated single-image path, with upright square UVs, no polygon scrolling/layout, and the source's sprite material selector. An asset-driven GPU test checks the image for both animation frames at size bytes 0, 255 and 251. The old path failed with 5,481 of 10,816 checked pixels differing at size byte 255; the corrected path passes all six cases. This checks rendering against source texture assets, without running a ROM oracle. |
| Arwing shadows absent in tunnel and elsewhere | The previous request to disable checkerboards had selected `Disabled`, suppressing all projected shadows. Application defaults and `starfox.ini` now select `Smooth`; explicit disabled and retail-checkerboard options remain available. A GPU regression renders an Arwing above a neutral lit floor, verifies darkening versus disabled shadows, and requires solid 2-by-2 covered blocks rather than checkerboard gaps. The diagnostic HD probe follows the updated shipping default. |
| Giant smoke/burn-mark images during gameplay | Fixed an asset-identity error: `makesmoke_srou` selected fire/burn-mark shape 357 instead of smoke shape 358. The wrong shape's extent is 188 rather than 40, approximately 4.7 times the intended width. `USHAPES.ASM` and the owned Rev-2 ROM header at source word ADD5 (file offset 11733) independently identify smoke's material, coordinate shift and extents. Corrected strategy tests and the oracle's direct-shape normalization, which had repeated the same mistaken alias. A cross-crate contract now follows actual smoke allocation into the generated render asset and metrics. No native expected trace was regenerated to conceal this discrepancy. |
| Oversized HD sprites near the camera | Restored `MDSPRITE.MC`'s near rejection at depth 128 and projected-size cap 240 in the HD billboard path. Source rasterization retains its existing exact projection. Unit and GPU tests cover depths 64, 128, 129 and 256; the prior single-bitmap tests still pass. |
| Textured polygon scroll wrap | Fixed wrapping individual UV vertices before interpolation. Authored coordinates plus scroll now remain continuous through a byte boundary; fragment sampling applies the original mask afterward. A unit test preserves the short 250-to-258 span instead of the incorrect 250-to-2 span. This is a separate defect, not the static `bu_7` logo cause. |

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
gameplay interval. It uses the shipping default of smooth shadows and smooth
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

- After the smoke/logo/timing follow-up, all 473 `sf-game`, `sf-render`
  and `sf-audio` tests passed and `sf-app` built. The GPU logo test covers
  45 poses at 1280 by 720 with smooth shading, including fractional camera
  rotation. The same test failed before the contained-decal repair. Log:
  `/tmp/sf1-smoke-logo-timing-tests-20260904.log`. The smoke asset contract
  and 34 focused strategy tests also pass. Native architecture and diff
  whitespace checks passed. No full ROM oracle run was needed for these
  source- and asset-backed repairs; the five-second stall remains partially
  unresolved rather than certified fixed.
- After the bitmap/shadow follow-up, all 232 `sf-render` tests passed,
  including the six-case bitmap readback regression and the smooth-shadow
  test. The shadow test also passed after strengthening its spatial coverage
  assertion. App config tests (2), app build, HD probe compile check, native
  architecture check and `git diff --check` passed. Renderer suite log:
  `/tmp/sf1-bitmap-shadow-tests-20260904.log`.
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
