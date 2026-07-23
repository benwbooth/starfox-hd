# boss2 (Macbeth spinning top) + bossg (sea boss) tick audit findings (2026-07-07, ASM-verified)
Scope: tick state machines only (inits previously verified). ASM refs authoritative:
GBSTRATS.ASM (boss2 family), D2STRATS.ASM (bossg), STRATMAC.INC / STRATLIB.INC macros.
Rust: rust/sf-strat/src/bosses.rs. Do not commit fixes without re-running boss fights.

Note for fixers: `ifeq 0` in this assembler ASSEMBLES the block (IFEQ expr = true when
expr==0). The turret children and the whole boss2plasma_strat body are LIVE in ROM;
the Rust port already treats them as live — correct, do not "fix" that.

## High
1. ~~m_bossHP per-frame accumulator missing~~ **FIXED (verified tick 141):**
   `add_bosshp` at boss2top / boss2turret / bossg_move2 / boss8_cont / boss1_fin;
   shell feeds `boss_hp_cur: v.bosshp`. Tests `boss_ticks1_verify.rs`.

2. ~~boss2 muzzle offsets ignore firer rotx/rotz~~ **FIXED (verified tick 141):**
   `b2_spawn_shot` → `b2_full_offset_pos` (rotz→rotx→roty). Tests
   `boss_ticks1_verify.rs`.

## Medium
3. ~~boss2 state-4 laser spread~~ **FIXED (verified tick 141):**
   `(rnd & 7) - 3` via `boss2_fire_relfastelaser` masks. (Covered by fire path.)

4. ~~boss2top missile coin flip~~ **FIXED (verified tick 141):**
   `rnd >= 127` → +deg22. Tests `boss_ticks1_verify.rs`.

5. ~~bossg sea_not_delay~~ **FIXED (verified tick 141):**
   `((gameframe + offset) & ((1<<n)-1)) != 0`; bossg sites pass offset 0.
   Splash gate + HP regen use bit masks. Tests `boss_ticks1_verify.rs`.

## Minor
6. ~~boss2 Zdistmore off-by-one~~ **FIXED (verified tick 141):** state0 smoke at
   `|dz| >= 1100` (near is `<`); state3 advance `>= 1100`; state4 hold `< 500`.
   Tests `boss_ticks1_verify.rs`.

7. ~~boss2petal death drop misses colldisable~~ **FIXED (verified tick 141):**
   `s_kill_obj` sets `ASF_COLLDISABLE`. Tests `boss_ticks1_verify.rs`.

8. ~~boss2 state-4 circle velocities floor negatives~~ **FIXED (verified tick 141):**
   `/8` and `/2` toward zero. Tests `boss_ticks1_verify.rs`.

9. ~~bossgs shadow-clone flicker~~ **FIXED (verified tick 142):**
   odd gameframe → `coltab = BLACK_C` (id 6); even → clear. Tests
   `boss_ticks1_gaps.rs`.

10. ~~bossgs x-chase clamps~~ **FIXED (verified tick 142):**
    Fchase_A ±5 with no overshoot clamp (oscillates within ±5 of sword1).
    Tests `boss_ticks1_gaps.rs`.

## Known gaps (pre-existing placeholders, not new findings)
- ~~bossg .genspark is a stub~~ **FIXED (tick 142):** copy-equivalent
  `worldy -= 60` then `player::sgen_spark` (D2STRATS.ASM:343-352 /
  `sgenspark_srou_l`). Tests `boss_ticks1_gaps.rs`.
- ~~bossg .move2 splash~~ **FIXED (tick 141):** even frames bump worldz+30,
  `makessplash_srou` + force splash worldy=0, restore −30 (D2STRATS.ASM:373-378).
  Tests `boss_ticks1_verify.rs`.
- ~~.scrollmsg only advances the tx counter~~ **VERIFIED (tick 143):**
  ROM `.scrollmsg` (D2STRATS.ASM:307-318) *is* `al_tx += 4` (texture U scroll)
  + optional z bump + mode advance on `tx & 127 == 0`. No separate message
  scroller. Tests `boss_ticks1_placeholders.rs`.
- ~~boss2 particlefiredown_Istrat is a placeholder tick~~ **FIXED (tick 143):**
  state-1 leap spawns nullshape with `particlefiredown_istrat` (payload 3/4/9,
  ASF_PARTOBJ + AFEXP + colldisable) per GBSTRATS.ASM:580. Tests
  `boss_ticks1_placeholders.rs`.

## Kamimissile (AUDIT_BOSS_TICKS2 leftover)
- ~~b8_fire_kamimissile speed~~ **FIXED (tick 141):** vel=40 / HP=2 / AP=8 / life=100
  match `fire_kamiHmissile1` (GSTRATS.ASM:2682-2693).
- ~~simplified boss8_kamimissile_strat~~ **FIXED (tick 144):** now delegates to
  `fire_kami_hmissile1` / `hmissile3_*` (laser weave + Z-band aim) with launcher
  post-fire `al_ptr`/`sflag1` (GASTRATS.ASM:103-110). Tests
  `boss8_kami_hmissile3.rs`.

## Verified correct (don't touch)
- boss2 state-machine shape: sequential fall-through == ROM (ASM `nextstate`
  STRATROU.ASM:2977 re-enters the strat top after s_next_state; same-tick chaining and
  the single trailing roty+=2 match Rust's if-chain + return placement).
- boss2 gameframe masks: state-4 fire &1, state-5 hitflash &1 (even frames), top laser
  &7, top missile &31, petal anim &3; petal top-death `s_jmp_NOTdelay 0,...,al1pt` has
  mask 0 -> never branches -> Rust's unconditional kill is right.
- state 2: s_jmp_lower #-1000 polarity (chase block only while worldy < -1000).
