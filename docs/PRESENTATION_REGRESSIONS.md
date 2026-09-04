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
| Missing intro siren and emergency voice | Under source/audio-event investigation. |
| Missing rectangular tunnel outline | Under source shape/draw-list investigation. |
| Missing blue smoke when wingmen launch | Under source strategy/asset investigation. |

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
