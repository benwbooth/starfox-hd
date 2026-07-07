//! Unit tests for game-core paths the level1_1 trace fixture doesn't reach:
//! compact spawn opcodes, external-var opcodes, jump-compare wraparound,
//! builtin spacebar strategies, collision damage rules and list-order
//! invariants. Expected values are hand-derived from the cited C code.

use sf_game::alien::*;
use sf_game::alien_compat as compat;
use sf_game::vars::{
    HARD_AP, HARD_HP, PALFADE_GROUND, PALFADE_NIGHT, PALFADE_NUM_START, PALFADE_SEA,
    PSF3_INTUNNEL,
};
use sf_game::world::op;
use sf_game::Game;
use sf_map::levels::BuiltLevel;

fn level_from_bytes(data: Vec<u8>) -> BuiltLevel {
    BuiltLevel {
        data,
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}

fn game_with(bytes: Vec<u8>) -> Game {
    let mut g = Game::new();
    g.load_level(&level_from_bytes(bytes));
    g
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
    assert_eq!(g.vars.palfade_from, PALFADE_NIGHT);
    assert_eq!(g.vars.palfade_target, PALFADE_SEA);
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START);
    // fadepalto_l runs once per frame (TRANS.ASM:167): -2 per tick.
    for i in 1..=15u16 {
        g.tick();
        assert_eq!(g.vars.palfade_num, PALFADE_NUM_START - 2 * i);
    }
    // Fade complete; further ticks hold it there (ROM: palnum==0 -> rtl).
    g.tick();
    assert_eq!(g.vars.palfade_num, 0);
    assert_eq!(g.vars.palfade_target, PALFADE_SEA);
}

#[test]
fn op_fadetoground_reverses_from_sea() {
    // WORLD.ASM:384-394 fadetogrounddo: same walk toward groundpal
    // (palfade = groundpal-seapal+30 = 62). The port records the previous
    // target as the fade source, so sea -> ground reverses the sea fade.
    let mut bytes = vec![op::FADETOSEA, op::FADETOGROUND];
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    assert_eq!(g.vars.mapptr, 5); // both fade ops (+2) then the mapwait (+3)
    assert_eq!(g.vars.palfade_from, PALFADE_SEA);
    assert_eq!(g.vars.palfade_target, PALFADE_GROUND);
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START);
    g.tick();
    assert_eq!(g.vars.palfade_num, PALFADE_NUM_START - 2);
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
    // C world.c:1119: shape word resolved, al_ptr = sub-map ref,
    // al_type = ATZREMOVE.
    let mut bytes = vec![op::MOTHER];
    bytes.extend_from_slice(&0u16.to_le_bytes()); // frame
    bytes.extend_from_slice(&5i16.to_le_bytes()); // x
    bytes.extend_from_slice(&6i16.to_le_bytes()); // y
    bytes.extend_from_slice(&7i16.to_le_bytes()); // z
    bytes.extend_from_slice(&241u16.to_le_bytes()); // raw boss_7_1 word
    bytes.extend_from_slice(&[0, 0, 0]); // strat addr24 (unregistered)
    bytes.extend_from_slice(&0x00CDu16.to_le_bytes()); // map ref
    let mut g = game_with(bytes);
    g.map_exec();
    let al = &g.objs.aliens[0];
    assert_eq!(al.shape, 56); // Shapes_ResolveShapeWord(241)
    assert_eq!(al.ptr, 0x00CD);
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
    // C world.c:1608: extptr @+1, lo @+4, hi @+6 (quirky order kept).
    let mut bytes = vec![op::SETVARL];
    bytes.extend_from_slice(&0x0400u16.to_le_bytes());
    bytes.push(0); // bank
    bytes.extend_from_slice(&0xBEEFu16.to_le_bytes());
    bytes.push(0x12);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    assert_eq!(g.vars.ram[0x0400], 0xEF);
    assert_eq!(g.vars.ram[0x0401], 0xBE);
    assert_eq!(g.vars.ram[0x0402], 0x12);
}

#[test]
fn op_jmpvar_uses_wrapped_signed_diff() {
    // C world.c:1846: diff = (int8)(ext - cmp). ext=5, cmp=250 ->
    // 5-250 wraps to 11 -> diff > 0, so JMPVARMORE jumps and
    // JMPVARLESS falls through.
    let mut bytes = vec![op::JMPVARLESS];
    bytes.extend_from_slice(&0x0500u16.to_le_bytes());
    bytes.push(0); // bank
    bytes.push(250); // cmp
    bytes.extend_from_slice(&100u16.to_le_bytes()); // target (not taken)
    bytes.push(op::JMPVARMORE); // offset 7
    bytes.extend_from_slice(&0x0500u16.to_le_bytes());
    bytes.push(0);
    bytes.push(250);
    bytes.extend_from_slice(&200u16.to_le_bytes()); // target (taken)
    let mut g = game_with(bytes);
    g.vars.ram[0x0500] = 5;
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
    bytes.extend_from_slice(&0x0600u16.to_le_bytes());
    bytes.push(0);
    bytes.push(op::ADDALVARPW);
    bytes.extend_from_slice(&38u16.to_le_bytes());
    bytes.extend_from_slice(&0x0600u16.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.vars.write_ext16(0x0600, (-7i16) as u16);
    g.map_exec();
    assert_eq!(g.objs.aliens[0].sword1, -14);
}

#[test]
fn op_setvarobj_stores_lastmapobj_encoding() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[op::QOBJ, 0, 0, 0, 0, 10, 0]);
    bytes.push(op::SETVAROBJ);
    bytes.extend_from_slice(&0x0700u16.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[op::WAIT, 0xE8, 0x03]);
    let mut g = game_with(bytes);
    g.map_exec();
    // Slot 0 spawned -> lastmapobj = idx+1 = 1 (C world.c:955).
    assert_eq!(g.vars.read_ext16(0x0700), 1);
}

// ---- builtin spacebar strategies (C world.c:177-325) ----

#[test]
fn spacebar_istrat_sets_hardvars_then_scrolls() {
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
    assert_ne!(al.collflags & 0x01, 0); // COLLTYPE_ENEMY1
    let z0 = al.worldz;

    // Second tick scrolls by pviewvelz.
    g.vars.pviewvelz = 7;
    g.run_strategies();
    assert_eq!(g.objs.aliens[0].worldz, z0 + 7);
}

#[test]
fn spinspacebar_chases_roty_toward_zero() {
    let mut bytes = vec![op::MAPOBJ];
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.extend_from_slice(&0i16.to_le_bytes());
    bytes.push(20);
    bytes.push(167); // MAP_ISTRAT_SPINSPACEBAR
    bytes.push(op::END);
    let mut g = game_with(bytes);
    g.map_exec();
    g.objs.aliens[0].sbyte1 = 3;
    g.objs.aliens[0].roty = 100;
    g.run_strategies(); // init tick
    g.run_strategies(); // spin tick: rotz += 3, roty -= 100>>3 = 12
    let al = &g.objs.aliens[0];
    assert_eq!(al.rotz, 3);
    assert_eq!(al.roty, 88);
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
    (a, b)
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
    // C obj.c:202-209: asf_collide with no collstratptr just clears.
    let mut g = game_with(vec![op::END]);
    g.map_exec();
    let a = g.objs.alloc().unwrap();
    g.objs.aliens[a as usize].hp = 5;
    g.objs.aliens[a as usize].sflags = ASF_COLLIDE;
    g.run_strategies();
    assert_eq!(g.objs.aliens[a as usize].sflags & ASF_COLLIDE, 0);
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
