//! Unit tests for game-core paths the level1_1 trace fixture doesn't reach:
//! compact spawn opcodes, external-var opcodes, jump-compare wraparound,
//! builtin spacebar strategies, collision damage rules and list-order
//! invariants. Expected values are hand-derived from the cited C code.

use sf_core::scene::PaletteFadeTarget;
use sf_game::alien::*;
use sf_game::alien_compat as compat;
use sf_game::vars::{HARD_AP, HARD_HP, PALFADE_NUM_START, PSF3_INTUNNEL};
use sf_game::world::{op, InlineCb};
use sf_game::Game;
use sf_game::Hooks;
use sf_map::consts::{sh, wm};
use sf_map::levels::BuiltLevel;
use std::cell::RefCell;
use std::rc::Rc;

fn level_from_bytes(data: Vec<u8>) -> BuiltLevel {
    BuiltLevel {
        data,
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}

#[test]
fn loading_a_level_clears_both_stage_special_counters() {
    let mut game = Game::new();
    game.vars.shared.specials_dead = 7;
    game.world.specialobjtotal = 9;
    game.world.total_specials = 9;

    game.load_level(&level_from_bytes(vec![op::END]));

    assert_eq!(game.vars.shared.specials_dead, 0);
    assert_eq!(game.world.specialobjtotal, 0);
    assert_eq!(game.world.total_specials, 0);
}

fn game_with(bytes: Vec<u8>) -> Game {
    let mut g = Game::new();
    g.load_level(&level_from_bytes(bytes));
    g
}

#[test]
fn alternate_music_command_has_the_retail_two_byte_semantics() {
    const TRACK: u8 = 42;

    struct MusicHooks(Rc<RefCell<Vec<u8>>>);
    impl Hooks for MusicHooks {
        fn play_music(&mut self, track_id: u8) {
            self.0.borrow_mut().push(track_id);
        }
    }

    let played = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(MusicHooks(played.clone())));
    game.load_level(&level_from_bytes(vec![op::SETBGM_ALIAS, TRACK, op::END]));
    game.map_exec();

    assert_eq!(*played.borrow(), vec![TRACK]);
    assert_eq!(
        game.vars.mapptr, 2,
        "the command consumes its track operand"
    );
}

#[test]
fn typed_mapend_preserves_level_finished_code() {
    let [lo, hi] = wm::LEVELFINISHED.to_le_bytes();
    let mut g = game_with(vec![op::SETVARB, 6, lo, hi, 0, op::END]);
    g.map_exec();
    assert_eq!(g.world.levelfinished, 6);
}

#[test]
fn banked_setvar_updates_meters_without_aliasing_typed_game_state() {
    // MAPMACS `meters_off/on` writes the low byte of m_meters at $70:0200.
    // The bank byte is semantically significant; this target is decoded
    // directly to the typed meter field.
    let mut g = game_with(vec![op::SETVARB, 1, 0x00, 0x02, 0x70, op::END]);
    g.map_exec();
    assert_eq!(g.vars.meters, 1);
    assert_eq!(g.vars.map.skill_fly, 0);
}

#[test]
fn setvar_numendok_updates_the_native_the_end_counter() {
    let [lo, hi] = wm::NUMENDOK.to_le_bytes();
    let mut g = game_with(vec![op::SETVARB, 0, lo, hi, 0, op::END]);
    g.vars.numendok = 0xFF;
    g.map_exec();
    assert_eq!(g.vars.numendok, 0);
}

#[test]
fn special_the_end_gate_yields_until_all_letters_finish() {
    let level = BuiltLevel {
        data: vec![op::CODE65816, op::WAIT2, 0],
        labels: vec![
            ("special.theenddead_check".to_string(), 0),
            ("special.theenddead_cont".to_string(), 1),
        ],
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    };

    let mut g = Game::new();
    g.load_level(&level);
    g.world
        .register_named_callbacks(&[], &[(1, "special_theenddead_check")], &level.labels);
    assert_eq!(
        g.world.find_inline(1),
        Some(InlineCb::SpecialTheEndGate {
            loop_ptr: 0,
            cont_ptr: 1,
        })
    );

    g.map_exec();
    assert_eq!(g.vars.mapptr, 0);
    assert_eq!(g.vars.mapcnt, 1);

    g.vars.numendok = 0xFF;
    g.vars.mapcnt = 0;
    g.map_exec();
    assert_eq!(g.vars.mapptr, 3);
}

#[test]
fn special_boss_cleanup_restores_player_flags_and_hides_meters() {
    let level = BuiltLevel {
        data: vec![op::CODE65816, op::WAIT2, 0],
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    };
    let mut g = Game::new();
    g.load_level(&level);
    g.world
        .register_named_callbacks(&[], &[(1, "special_boss_cleanup")], &[]);
    g.vars.meters = 1;
    g.vars.pshipflags = sf_game::vars::PSF_NOFIRE;
    g.vars.pstratflags = sf_game::vars::PSTF_NOTDIE;
    g.map_exec();
    assert_eq!(g.vars.meters, 0);
    assert_eq!(g.vars.pshipflags & sf_game::vars::PSF_NOFIRE, 0);
    assert_eq!(g.vars.pstratflags & sf_game::vars::PSTF_NOTDIE, 0);
}

#[test]
fn training_ring_gate_skips_or_repeats_at_fifteen_rings() {
    // The source macro is CODE65816 followed by mapgoto .et.  Start directly
    // on the gate; each destination parks on WAIT2 so the resulting pointer is
    // mechanically observable after one map_exec call.
    let level = BuiltLevel {
        data: vec![op::WAIT2, 1, op::CODE65816, op::GOTO, 0, 0, 0, op::WAIT2, 1],
        labels: vec![("training.eguchifly_continue".to_string(), 7)],
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    };

    let run = |rings: u16| {
        let mut g = Game::new();
        g.load_level(&level);
        g.world
            .register_named_callbacks(&[], &[(3, "training_eguchifly_check")], &level.labels);
        g.vars.mapptr = 2;
        g.vars.write_ext16(0x2300, rings);
        g.map_exec();
        g.vars.mapptr
    };

    assert_eq!(run(14), 9, "14 rings skips the course-repeat GOTO");
    assert_eq!(run(15), 2, "15 rings executes the course-repeat GOTO");
    assert_eq!(run(999), 2, "the source uses an unsigned >= 15 test");
}

// ---- palette fade opcodes ----

#[test]
fn op_fadetosea_arms_walk_and_ticks_to_full() {
    // WORLD.ASM:371-380 fadetoseado: palfade=lastpalfade=30, palcnt=2,
    // palnum=30; MAIN.ASM:2762 fadepalto_l then copies one seapal color
    // per frame and steps palnum -2 until 0 (15 frames, colors 15..1).
    let mut bytes = vec![op::FADETOSEA];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]); // park the VM
    let mut g = game_with(bytes);
    g.map_exec();
    assert_eq!(g.vars.mapptr, 4); // fade op (+1) then the parked mapwait (+3)
    assert_eq!(g.vars.palfade_target, Some(PaletteFadeTarget::Sea));
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START);
    // fadepalto_l runs once per frame (TRANS.ASM:167): -2 per tick.
    for i in 1..=15u16 {
        g.tick();
        assert_eq!(g.vars.palfade_num, PALFADE_NUM_START - 2 * i);
    }
    // Fade complete; further ticks hold it there (ROM: palnum==0 -> rtl).
    g.tick();
    assert_eq!(g.vars.palfade_num, 0);
    assert_eq!(g.vars.palfade_target, Some(PaletteFadeTarget::Sea));
}

#[test]
fn op_fadetoground_retargets_the_palette_walk() {
    // WORLD.ASM:384-394 fadetogrounddo: same walk toward groundpal
    // (palfade = groundpal-seapal+30 = 62). The renderer retains the live
    // background row, so entries not reached by the new walk remain sea.
    let mut bytes = vec![op::FADETOSEA, op::FADETOGROUND];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    assert_eq!(g.vars.mapptr, 5); // both fade ops (+2) then the mapwait (+3)
    assert_eq!(g.vars.palfade_target, Some(PaletteFadeTarget::Ground));
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START);
    g.tick();
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START - 2);
}

#[test]
fn op_waitfade_parks_while_fade_active() {
    // WORLD.ASM:726-740 mapwaitfadedo: while fade active, mapcnt=1 and stay;
    // when idle, advance past the opcode.
    struct FadeHooks {
        active: bool,
    }
    impl sf_game::Hooks for FadeHooks {
        fn is_map_fade_active(&self) -> bool {
            self.active
        }
    }

    let mut bytes = vec![op::WAITFADE];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let level = level_from_bytes(bytes.clone());

    // Active fade: park with mapcnt=1, mapptr unchanged at WAITFADE.
    let mut g = Game::with_hooks(Box::new(FadeHooks { active: true }));
    g.load_level(&level);
    g.map_exec();
    assert_eq!(g.vars.mapptr, 0);
    assert_eq!(g.vars.mapcnt, 1);

    // Idle fade: advance past WAITFADE into the following mapwait.
    let mut g = Game::with_hooks(Box::new(FadeHooks { active: false }));
    g.load_level(&level_from_bytes(bytes));
    g.map_exec();
    assert_eq!(g.vars.mapptr, 4); // waitfade (+1) then parked mapwait (+3)
    assert_eq!(g.vars.mapcnt, 0x03E8);
}

// ---- VOFS please (WORLD.ASM vofson/offplease) ----

#[test]
fn op_vofson_latches_bg2scroll_and_enables() {
    // WORLD.ASM:1157-1166: bg2vofs=bg2scroll, dovofs=1, bgmode=2.
    let mut bytes = vec![op::VOFSON];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.vars.shared.background_scroll = 232;
    g.map_exec();
    assert_eq!(g.vars.mapptr, 4);
    assert_eq!(g.vars.bg2vofs, 232);
    assert_eq!(g.vars.dovofs, 1);
    assert_eq!(g.vars.bgmode, 2);
}

#[test]
fn op_vofsoff_clears_dovofs_mode1() {
    // WORLD.ASM:1180-1190: dovofs=0, bgmode=1, still latch bg2vofs.
    let mut bytes = vec![op::VOFSON, op::VOFSOFF];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.vars.shared.background_scroll = 488;
    g.map_exec();
    assert_eq!(g.vars.mapptr, 5);
    assert_eq!(g.vars.dovofs, 0);
    assert_eq!(g.vars.bgmode, 1);
    assert_eq!(g.vars.bg2vofs, 488);
}

#[test]
fn vofs_please_l_direct() {
    let mut g = Game::new();
    g.vars.shared.background_scroll = 100;
    g.vars.vofs_on_please();
    assert_eq!(g.vars.dovofs, 1);
    assert_eq!(g.vars.bgmode, 2);
    assert_eq!(g.vars.bg2vofs, 100);
    g.vars.shared.background_scroll = 50;
    g.vars.vofs_off_please();
    assert_eq!(g.vars.dovofs, 0);
    assert_eq!(g.vars.bgmode, 1);
    assert_eq!(g.vars.bg2vofs, 50);
}

#[test]
fn op_hofson_off_latches_dohofs() {
    // WORLD.ASM:1195-1206: dohofs=1 / stz dohofs.
    let mut bytes = vec![op::HOFSON, op::HOFSOFF];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    // After HOFSON then HOFSOFF in one map_exec (both continue), dohofs=0.
    assert_eq!(g.vars.dohofs, 0);
    assert_eq!(g.vars.mapptr, 5);
    let mut g = game_with(vec![op::HOFSON, op::WAIT, 0xE8, 0x03]);
    g.map_exec();
    assert_eq!(g.vars.dohofs, 1);
}

// ---- spawn opcodes ----

#[test]
fn op_qobj_decompression() {
    // C world.c:1686 mapqobjdo: mapcnt = frame<<4, x/y sign-extended <<2,
    // z ZERO-extended <<4.
    let mut g = game_with(vec![op::QOBJ, 2, 0xFC, 5, 0xF0, 10, 0]);
    g.map_exec();
    assert_eq!(g.vars.mapcnt, 32);
    assert_eq!(g.vars.mapptr, 7);
    let al = &g.objs.aliens[0];
    assert!(al.active);
    assert_eq!(al.worldx, -16);
    assert_eq!(al.worldy, 20);
    assert_eq!(al.worldz, 0x0F00); // 0xF0 << 4, unsigned
    assert_eq!(al.shape, 10);
    assert_eq!(al.stratptr, None); // empty strategy table -> inert
    assert_eq!(al.animframe, 0); // ROM init_objvars_l zeroes the block (bit7 clear = animate)
    assert_eq!(al.collflags, ACF_FIRSTFRAME);
}

#[test]
fn op_obj8_sign_extends_z() {
    // C world.c:1714: obj8 z is sign-extended ((int16)z_raw << 4),
    // and mapcnt = frame<<2.
    let mut g = game_with(vec![op::OBJ8, 3, 0, 0, 0xF0, 56, 0, 0x34, 0x12, 3]);
    g.map_exec();
    assert_eq!(g.vars.mapcnt, 12);
    assert_eq!(g.vars.mapptr, 10);
    let al = &g.objs.aliens[0];
    assert_eq!(al.worldz, -256); // (i8)0xF0 = -16, <<4
    assert_eq!(al.shape, 56);
    // strat addr 0x031234 unregistered -> inert.
    assert_eq!(al.stratptr, None);
}

#[test]
fn op_mother_sets_ptr_and_zremove() {
    // WORLD.ASM mapmother: the flat source-catalog shape id is copied into
    // the actor, along with the typed sub-map reference and lifetime policy.
    const MOTHER_MAP_REF: u16 = 205;
    let mut bytes = vec![op::MOTHER];
    bytes.extend_from_slice(&0u16.to_le_bytes()); // frame
    bytes.extend_from_slice(&5i16.to_le_bytes()); // x
    bytes.extend_from_slice(&6i16.to_le_bytes()); // y
    bytes.extend_from_slice(&7i16.to_le_bytes()); // z
    bytes.extend_from_slice(&sh::BOSS_7_1.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0]); // strat addr24 (unregistered)
    bytes.extend_from_slice(&MOTHER_MAP_REF.to_le_bytes());
    let mut g = game_with(bytes);
    g.map_exec();
    let al = &g.objs.aliens[0];
    assert_eq!(al.shape, sh::BOSS_7_1);
    assert_eq!(al.ptr, MOTHER_MAP_REF);
    assert_eq!(al.type_, ATZREMOVE);
    assert_eq!(g.vars.mapptr, 16);
}

#[test]
fn op_dobj_takes_shape_from_istrat_table() {
    // C world.c:1745: dobj shape = g_istrat_shapes[strat] (0 while the
    // strat lane is unported).
    let mut bytes = vec![op::DOBJ];
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1i16.to_le_bytes());
    bytes.extend_from_slice(&2i16.to_le_bytes());
    bytes.extend_from_slice(&3i16.to_le_bytes());
    bytes.push(42); // strat id
    let mut g = game_with(bytes);
    g.world.istrat_shapes[42] = 777;
    g.map_exec();
    assert_eq!(g.objs.aliens[0].shape, 777);
    assert_eq!(g.vars.mapptr, 10);
}

#[test]
fn op_remove_frees_matching_shapes() {
    // ROM mapremove (WORLD.ASM:1973-1993, oracle audit_mapvm2): removes exactly
    // ONE match per execution and never checks the player slot (0). Spawn
    // three: slot 0 (player-slot stand-in, shape 10), slots 1+2 (shape 10),
    // then REMOVE(10) — only the FIRST non-player match is freed.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.push(op::REMOVE);
    bytes.extend_from_slice(&0u16.to_le_bytes()); // frame (ignored)
    bytes.extend_from_slice(&10u16.to_le_bytes()); // shape
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]); // pause
    let mut g = game_with(bytes);
    g.map_exec();
    assert!(g.objs.aliens[0].active, "player slot exempt");
    let live: Vec<bool> = (1..3).map(|i| g.objs.aliens[i].active).collect();
    assert_eq!(
        live.iter().filter(|&&a| a).count(),
        1,
        "exactly one of the two non-player matches removed: {live:?}"
    );
}

#[test]
fn op_wait2_scales_by_16_and_pauses() {
    let mut g = game_with(vec![op::WAIT2, 2, op::WAIT2, 0, op::SETSTAGE]);
    g.map_exec();
    assert_eq!(g.vars.mapcnt, 32);
    assert_eq!(g.vars.mapptr, 2);
    // ROM mapwait2 (WORLD.ASM:175-187) RTSes unconditionally — a zero raw still
    // ends the frame (mapcnt=0), unlike mapwait (oracle audit_mapvm2).
    g.vars.mapcnt = 0;
    g.vars.mapptr = 2;
    g.map_exec();
    assert_eq!(g.vars.mapptr, 4, "wait2 0 still ends the frame");
    assert_eq!(g.vars.mapcnt, 0);
    assert_eq!(g.vars.stagecnt, 0, "SETSTAGE not reached this frame");
    // The next frame (mapcnt already 0) continues into SETSTAGE.
    g.map_exec();
    assert_eq!(g.vars.stagecnt, 50);
}

// ---- external-variable opcodes ----

#[test]
fn op_setvarl_literal_operand_order() {
    // Retained source-data decoder order: variable @+1, lo @+4, hi @+6.
    let mut bytes = vec![op::SETVARL];
    bytes.extend_from_slice(&wm::MAPVAR1.to_le_bytes());
    bytes.push(0); // bank
    bytes.extend_from_slice(&0xBEEFu16.to_le_bytes());
    bytes.push(0x12);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    assert_eq!(g.vars.map.variable1, 0x12_BEEF);
}

#[test]
fn op_jmpvar_uses_wrapped_signed_diff() {
    // C world.c:1846: diff = (int8)(ext - cmp). ext=5, cmp=250 ->
    // 5-250 wraps to 11 -> diff > 0, so JMPVARMORE jumps and
    // JMPVARLESS falls through.
    let mut bytes = vec![op::JMPVARLESS];
    bytes.extend_from_slice(&wm::SKILLFLY.to_le_bytes());
    bytes.push(0); // bank
    bytes.push(250); // cmp
    bytes.extend_from_slice(&100u16.to_le_bytes()); // target (not taken)
    bytes.push(op::JMPVARMORE); // offset 7
    bytes.extend_from_slice(&wm::SKILLFLY.to_le_bytes());
    bytes.push(0);
    bytes.push(250);
    bytes.extend_from_slice(&200u16.to_le_bytes()); // target (taken)
    let mut g = game_with(bytes);
    g.vars.map.skill_fly = 5;
    g.map_exec();
    assert_eq!(g.vars.mapptr, 200);
}

#[test]
fn op_setalvarp_and_addalvarp_read_wram() {
    // Spawn, then setalvarpw al_sword1(38) from WRAM, then addalvarpw
    // adds the same word again (C world.c:1495/1656).
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.push(op::SETALVARPW);
    bytes.extend_from_slice(&38u16.to_le_bytes()); // al_sword1 offset
    bytes.extend_from_slice(&wm::HPOSJMP.to_le_bytes());
    bytes.push(0);
    bytes.push(op::ADDALVARPW);
    bytes.extend_from_slice(&38u16.to_le_bytes());
    bytes.extend_from_slice(&wm::HPOSJMP.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.vars.map.horizontal_position_jump = -7;
    g.map_exec();
    assert_eq!(g.objs.aliens[0].sword1, -14);
}

#[test]
fn op_setvarobj_stores_lastmapobj_encoding() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.push(op::SETVAROBJ);
    bytes.extend_from_slice(&wm::BOSSMAXHP.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    // Slot 0 spawned -> lastmapobj = idx+1 = 1 (C world.c:955).
    assert_eq!(g.vars.bossmaxhp, 1);
}

#[test]
fn background_ids_publish_rom_sound_environment() {
    let mut v = sf_game::vars::GameVars::default();
    v.in_a_tunnel = 2;
    v.set_sound_environment_for_bg(4); // planet
    assert_eq!(v.in_a_tunnel, 0);
    v.set_sound_environment_for_bg(8); // tunnel
    assert_eq!(v.in_a_tunnel, 1);
    v.set_sound_environment_for_bg(24); // water/colony
    assert_eq!(v.in_a_tunnel, 2);
    v.set_sound_environment_for_bg(2); // blink: no terminal macro
    assert_eq!(v.in_a_tunnel, 2, "blink retains the prior environment");
}

// ---- builtin spacebar strategies (C world.c:177-325) ----

#[test]
fn spacebar_istrat_sets_hardvars_then_spacemist() {
    // mapobj frame=0 strat=166 then END.
    let mut bytes = vec![op::MAPOBJ];
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&100i16.to_le_bytes());
    bytes.push(20); // shape byte
    bytes.push(166); // MAP_ISTRAT_SPACEBAR
    bytes.push(op::END);
    let mut g = game_with(bytes);
    g.map_exec();
    let al = &g.objs.aliens[0];
    assert!(al.stratptr.is_some(), "istrat init assigned");
    assert_eq!(al.hp, 0, "hardvars only apply on first strategy tick");

    // First strategy tick: init runs -> hardvars + tick strat installed.
    g.run_strategies();
    let al = &g.objs.aliens[0];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, HARD_AP);
    assert_ne!(al.collflags & 0x10, 0); // ACF_COLLTYPE2 = ROM ENEMY1
    let z0 = al.worldz;

    // Second tick: ROM spacebar_strat is spacemist only (no add_playerZ).
    g.vars.pviewvelz = 7;
    g.run_strategies();
    assert_eq!(
        g.objs.aliens[0].worldz, z0,
        "must not scroll with pviewvelz"
    );
    assert_ne!(g.objs.aliens[0].colframe & 0x80, 0, "spacemist sets hi bit");
}

#[test]
fn spinspacebar_chases_roty_toward_zero() {
    let mut bytes = vec![op::MAPOBJ];
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.push(20);
    bytes.push(166); // MAP_ISTRAT_SPINSPACEBAR
    bytes.push(op::END);
    let mut g = game_with(bytes);
    g.map_exec();
    g.objs.aliens[0].sbyte1 = 3;
    g.objs.aliens[0].roty = 100;
    g.run_strategies(); // init tick
    let z0 = g.objs.aliens[0].worldz;
    g.vars.pviewvelz = 5;
    g.run_strategies(); // spin tick: rotz += 3, roty -= 100>>3 = 12
    let al = &g.objs.aliens[0];
    assert_eq!(al.rotz, 3);
    assert_eq!(al.roty, 88);
    assert_eq!(al.worldz, z0, "SPINspacebar has no add_playerZ");
    assert_ne!(al.colframe & 0x80, 0);
}

#[test]
fn achase_angle_8_antipodal_matches_rom() {
    // cur=0 tgt=128 rate1 → ROM steps toward 192 (not 64).
    let mut cur = 0u8;
    assert!(!sf_core::snes_trig::achase_angle_8(&mut cur, 128, 1));
    assert_eq!(cur, 192);
}

#[test]
fn spacebar1_spins_rotz_and_spacemist_no_scroll() {
    let mut bytes = vec![op::MAPOBJ];
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&200i16.to_le_bytes());
    bytes.push(20);
    bytes.push(167); // MAP_ISTRAT_SPACEBAR1
    bytes.push(op::END);
    let mut g = game_with(bytes);
    g.map_exec();
    g.objs.aliens[0].sbyte1 = 5;
    g.objs.aliens[0].sbyte2 = 0; // keep collision enabled
    g.run_strategies(); // init
    let z0 = g.objs.aliens[0].worldz;
    g.vars.pviewvelz = 9;
    g.run_strategies(); // tick
    let al = &g.objs.aliens[0];
    assert_eq!(al.rotz, 5);
    assert_eq!(al.worldz, z0);
    assert_ne!(al.colframe & 0x80, 0);
}

// ---- collision system (C src/game/coldet.c) ----

fn make_pair(g: &mut Game, ta: u8, tb: u8) -> (u16, u16) {
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    for (i, t) in [(a, ta), (b, tb)] {
        let al = &mut g.objs.aliens[i as usize];
        al.collflags = t; // clears ACF_FIRSTFRAME
        al.hp = 20;
        al.ap = 0;
    }
    // Distinct shapes: a real colliding pair (e.g. laser vs enemy) has different
    // al_shape. The ROM chkcoll0 same-shape gate skips equal-shape pairs, so
    // leaving both at the default 0 would (correctly) suppress the collision and
    // is not what these colltype/response tests are exercising.
    g.objs.aliens[a as usize].shape = 1;
    g.objs.aliens[b as usize].shape = 2;
    (a, b)
}

#[test]
fn coldet_same_shape_gate_skips_unless_sameshapecollide() {
    use sf_game::alien::ASF3_SAMESHAPECOLLIDE;
    let mut g = Game::new();
    // Collidable colltypes (different bits) but the SAME al_shape: ROM chkcoll0
    // skips the pair (retail-certified: coexec retail_same_shape_skip_divergence).
    let (a, b) = make_pair(&mut g, ACF_COLLTYPE1, ACF_COLLTYPE2);
    g.objs.aliens[a as usize].shape = 42;
    g.objs.aliens[b as usize].shape = 42;
    g.coldet_generate_list();
    g.coldet_run();
    assert_eq!(
        g.objs.aliens[a as usize].sflags & ASF_COLLIDE,
        0,
        "same-shape pair must NOT collide (ROM chkcoll0 gate)"
    );
    // With both objects flagged sameshapecollide, the gate is bypassed.
    g.objs.aliens[a as usize].sflags3 |= ASF3_SAMESHAPECOLLIDE;
    g.objs.aliens[b as usize].sflags3 |= ASF3_SAMESHAPECOLLIDE;
    g.coldet_generate_list();
    g.coldet_run();
    assert_ne!(
        g.objs.aliens[a as usize].sflags & ASF_COLLIDE,
        0,
        "sameshapecollide on both -> same-shape pair collides"
    );
}

#[test]
fn coldet_applies_ap_damage_with_cooldown() {
    let mut g = Game::new();
    let (a, b) = make_pair(&mut g, ACF_COLLTYPE1, ACF_COLLTYPE2);
    g.objs.aliens[a as usize].hp = 10;
    g.objs.aliens[a as usize].ap = 4;
    g.objs.aliens[b as usize].ap = 8;
    // Fresh objects have collcount == 0. On a *first-frame* collision the ROM
    // seeds collcount via init_strats_ram_l (COLDET.ASM:172-182): any object
    // not already colliding gets collcount = 1 at the top of coldet_run. do_coll
    // (do_coll_l, DEC-then-BNE) then DECs 1 -> 0, the BNE falls through, and
    // damage is applied. (An earlier stale version of this test seeded no
    // collcount and expected damage at collcount == 0, which the ROM-correct
    // do_coll never applies.)
    assert_eq!(g.objs.aliens[a as usize].collcount, 0);
    g.coldet_generate_list();
    assert_eq!(g.coldet.list.len(), 2);
    g.coldet_run();
    // First-frame collision: coldet_run reset collcount 0 -> 1, do_coll DEC'd
    // 1 -> 0 and applied damage. A took B's AP (10 - 8 = 2), B took A's AP
    // (20 - 4 = 16); both cooldowns latched to FRAMESPERAP.
    assert_eq!(g.objs.aliens[a as usize].hp, 2);
    assert_eq!(g.objs.aliens[b as usize].hp, 16);
    assert_eq!(g.objs.aliens[a as usize].collcount, 10); // FRAMESPERAP
    assert_ne!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(g.objs.aliens[a as usize].collobjptr, b);
    assert_eq!(g.objs.aliens[b as usize].collobjptr, a);

    // Second frame: still overlapping and still colliding, so init_strats_ram_l
    // does NOT reset collcount; do_coll DECs the latched 10 -> 9 and, being
    // nonzero, applies no damage (the AP cooldown).
    g.coldet_generate_list();
    g.coldet_run();
    assert_eq!(g.objs.aliens[a as usize].hp, 2);
    assert_eq!(g.objs.aliens[a as usize].collcount, 9);
    // collide got mirrored into Lcollide during step 1 then re-set.
    assert_ne!(g.objs.aliens[a as usize].sflags & ASF_LCOLLIDE, 0);
}

#[test]
fn coldet_tunnel_halves_hard_ap_and_hardhp_is_immune() {
    let mut g = Game::new();
    let (a, b) = make_pair(&mut g, ACF_COLLTYPE1, ACF_COLLTYPE2);
    g.vars.pshipflags3 |= PSF3_INTUNNEL;
    g.objs.aliens[a as usize].hp = 20;
    g.objs.aliens[b as usize].hp = HARD_HP; // indestructible
    g.objs.aliens[b as usize].ap = HARD_AP;
    g.objs.aliens[a as usize].ap = 5;
    // First-frame collision: coldet_run seeds collcount 0 -> 1 (init_strats_ram_l,
    // COLDET.ASM:172-182) so the first do_coll (DEC 1 -> 0) applies damage.
    assert_eq!(g.objs.aliens[a as usize].collcount, 0);
    g.coldet_generate_list();
    g.coldet_run();
    // In-tunnel hardAP is halved before do_coll (do_coll_l: 8 >> 1 = 4), so A
    // takes 20 - 4 = 16.
    assert_eq!(g.objs.aliens[a as usize].hp, 16);
    // B's hp has bit 7 set (hardHP): do_coll's `LDA hp; BMI` skips the subtract,
    // so hp is unchanged but the AP cooldown still latches to FRAMESPERAP.
    assert_eq!(g.objs.aliens[b as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[b as usize].collcount, 10);
}

#[test]
fn coldet_filters_same_type_immune_and_untyped() {
    let mut g = Game::new();
    // Same category: no collision.
    let (a, b) = make_pair(&mut g, ACF_COLLTYPE2, ACF_COLLTYPE2);
    g.coldet_generate_list();
    g.coldet_run();
    assert_eq!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);

    // Untyped objects DO collide. ROM chkcoll0 (COLDET.ASM:518-521) skips a
    // pair only when it shares a collision-type bit (cf1 & cf2 & typemask != 0);
    // it does NOT require either object to carry a type bit. The earlier port
    // (from src/game/coldet.c) added a spurious `a_types == 0 && b_types == 0 ->
    // skip`, which wrongly dropped objects that have collflags but no type bit.
    // (immuneptr set to a non-slot sentinel so the immunity check can't confound
    // this category-filter case — see the player-slot-0 note in coldet.rs.)
    g.objs.aliens[a as usize].collflags = ACF_WEAPON;
    g.objs.aliens[b as usize].collflags = ACF_WEAPON;
    g.objs.aliens[a as usize].immuneptr = 0xFFFF;
    g.objs.aliens[b as usize].immuneptr = 0xFFFF;
    g.coldet_generate_list();
    g.coldet_run();
    assert_ne!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);
    assert_ne!(g.objs.aliens[b as usize].sflags & ASF_COLLIDE, 0);

    // Immunity cross-reference: A immune to B -> skip (COLDET.ASM:523-529).
    g.objs.aliens[a as usize].collflags = ACF_COLLTYPE1;
    g.objs.aliens[b as usize].collflags = ACF_COLLTYPE2;
    g.objs.aliens[a as usize].immuneptr = b;
    g.objs.aliens[b as usize].immuneptr = 0xFFFF;
    g.coldet_generate_list();
    g.coldet_run();
    assert_eq!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn coldet_skips_firstframe_colldisable_hp0_exploding() {
    let mut g = Game::new();
    let a = g.objs.alloc().unwrap(); // collflags = ACF_FIRSTFRAME
    g.objs.aliens[a as usize].hp = 5;
    g.coldet_generate_list();
    assert!(g.coldet.list.is_empty(), "firstframe skipped");

    g.objs.aliens[a as usize].collflags = 0;
    g.objs.aliens[a as usize].sflags = ASF_COLLDISABLE;
    g.coldet_generate_list();
    assert!(g.coldet.list.is_empty(), "colldisable skipped");

    g.objs.aliens[a as usize].sflags = 0;
    g.objs.aliens[a as usize].hp = 0;
    g.coldet_generate_list();
    assert!(g.coldet.list.is_empty(), "hp 0 skipped");

    g.objs.aliens[a as usize].hp = 5;
    g.objs.aliens[a as usize].flags = AFEXP;
    g.coldet_generate_list();
    assert!(g.coldet.list.is_empty(), "exploding skipped");

    g.objs.aliens[a as usize].flags = 0;
    g.coldet_generate_list();
    assert_eq!(g.coldet.list.len(), 1);
    // Default extents without shape data (C DEFAULT_COLL_EXTENT).
    assert_eq!(g.coldet.list[0].xmax, 20);
}

// ---- object pool invariants (C src/game/obj.c) ----

#[test]
fn obj_free_list_orders_match_c() {
    let mut g = Game::new();
    // Obj_Init pushes 69..0 -> first alloc is 0, then 1, 2...
    assert_eq!(g.objs.alloc(), Some(0));
    assert_eq!(g.objs.alloc(), Some(1));
    assert_eq!(g.objs.alloc(), Some(2));
    // Free is LIFO: freeing 1 makes it the next allocation.
    g.objs.free(1);
    assert_eq!(g.objs.alloc(), Some(1));
    // Active list is push-front: head is the newest.
    assert_eq!(g.objs.active_indices(), vec![1, 2, 0]);

    // Obj_KillAll builds the free list FORWARD like the ROM (kill_list_l/
    // FmtFreeLst) -> head is slot 0, so the first alloc afterwards is 0.
    g.objs.kill_all();
    assert_eq!(g.objs.alloc(), Some(0));
}

#[test]
fn obj_pool_exhaustion_returns_none() {
    let mut g = Game::new();
    for _ in 0..NUMBER_AL {
        assert!(g.objs.alloc().is_some());
    }
    assert_eq!(g.objs.alloc(), None);
}

// ---- dostrats dispatch (C src/game/obj.c do_strat_l) ----

#[test]
fn do_strat_clears_firstframe_and_nuked() {
    let mut g = game_with(vec![op::END]);
    g.map_exec(); // levelfinished -> map VM inert
    let a = g.objs.alloc().unwrap();
    g.objs.aliens[a as usize].type_ = ATNUKED | ATGND;
    assert_ne!(g.objs.aliens[a as usize].collflags & ACF_FIRSTFRAME, 0);
    g.run_strategies();
    let al = &g.objs.aliens[a as usize];
    assert_eq!(al.collflags & ACF_FIRSTFRAME, 0);
    assert_eq!(al.type_, ATGND, "nuked bit cleared");
}

#[test]
fn do_strat_collide_without_callback_clears_flag() {
    fn tick(g: &mut Game, idx: u16) {
        g.objs.aliens[idx as usize].sbyte1 += 1;
    }

    // Do_strat_l clears a collide flag with no collstratptr, then branches to
    // `.ns` and dispatches the ordinary stratptr in the same pass.
    let mut g = game_with(vec![op::END]);
    g.map_exec();
    let a = g.objs.alloc().unwrap();
    let tick = g.world.register_strategy(tick);
    g.objs.aliens[a as usize].hp = 5;
    g.objs.aliens[a as usize].sflags = ASF_COLLIDE;
    g.objs.aliens[a as usize].stratptr = Some(tick);
    g.run_strategies();
    assert_eq!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(g.objs.aliens[a as usize].sbyte1, 1);
}

#[test]
fn dummyobj_skips_strategy_dispatch() {
    // C obj.c:177: cpx dummyobj -> skip (firstframe NOT cleared).
    let mut g = game_with(vec![op::END]);
    g.map_exec();
    let _p = g.objs.alloc().unwrap(); // slot 0
    let a = g.objs.alloc().unwrap(); // slot 1
    g.vars.dummyobj = a as i16;
    g.run_strategies();
    assert_ne!(g.objs.aliens[a as usize].collflags & ACF_FIRSTFRAME, 0);
    assert_eq!(g.objs.aliens[0].collflags & ACF_FIRSTFRAME, 0);
}

// ---- compat layer (C src/game/alien_compat.c) ----

#[test]
fn compat_denies_strategy_pointer_offsets() {
    let mut al = Alien::default();
    // al_stratptr bytes 22..24 denied.
    assert!(!compat::write8(&mut al, 22, false, 1));
    assert!(!compat::write8(&mut al, 24, false, 1));
    // alx strategy pointers 6..17 denied.
    for off in 6..=17 {
        assert!(!compat::write8(&mut al, off, true, 1));
        assert!(compat::read8(&al, off, true).is_none());
    }
    // Word write straddling a denied byte fails without the second byte.
    assert!(!compat::write16(&mut al, 21, false, 0x1234));
    // ...but the low byte landed first, exactly like C's sequential writes.
    assert_eq!(al.vel, 0x34);
}

#[test]
fn compat_word_lo_hi_semantics() {
    let mut al = Alien::default();
    assert!(compat::write16(&mut al, 38, false, 0xBEEF));
    assert_eq!(al.sword1, 0xBEEFu16 as i16);
    assert_eq!(compat::read16(&al, 38, false), Some(0xBEEF));
    // alx depthoffset lo/hi (21/22).
    assert!(compat::write8(&mut al, 21, true, 0x01));
    assert_eq!(al.depthoffset, 1);
    assert!(compat::add16(&mut al, 38, false, 0x0111));
    assert_eq!(al.sword1, 0xBEEFu16.wrapping_add(0x0111) as i16);
}
