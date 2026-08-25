//! amoeba swarm + amoebastick "stick-to-player" behavioral tests.
//!
//! ASM oracle: `amoeba_Istrat` / `amoeba_strat` / `amoebacol_Istrat` /
//! `amoebahome_init|strat` / `amoebastick_Istrat|strat` / `amoebago_init`
//! (reference/ultrastarfox/SF/STRAT/GA2STRAT.ASM:126-224). Macro semantics
//! per docs/AUDIT_BOSS_TICKS2_FINDINGS.md and STRATMAC.INC (cited inline).
//!
//! No sf-oracle differential fixture: the amoeba is mother-spawned and its
//! whole lifecycle reads/writes cross-object player globals (player_posx/y,
//! pcboxobj_B, player_rollZvel, pshipflags, slimecount) plus the ROM's
//! software-sprite path — none of which is a pure-65816-tractable single
//! function. These tests hand-derive expected values from the ASM.
//!
//! The dedicated `#amoeba1` windshield-splat sprite is compiled from
//! USHAPES.ASM into a stable native shape slot.
//!  * hardHP(0xFF) makes the body indestructible (do_coll BMI) and expstrat=0
//!    means NO death/explode path — the only removal is drift-off (ATZREMOVE)
//!    or a barrel-roll fling.

use sf_game::alien::{ObjectVisualKind, ASF2_COLLDISABLE, ASF_COLLIDE};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{pcbox_attach, strat_spawn_player};
use sf_strat::snes_trig::strat_roffs_full_i16;
use sf_strat::{bosses, table};

// ---- local mirrors of private bosses.rs / engine constants ----
const WM_RNDVAL: u16 = 0x1F00;
const AMOEBA_SLIMECOUNT: u16 = 0x162b; // GILESALC.INC:296
const SH_AMOEBA1: u16 = 438;
const PSF_NOFIRE: u8 = 64;
const DEG180: u8 = 128;
const HARD_HP: u8 = 0xFF;

/// Bare player object at slot 0 + amoeba armed with its registered istrat.
/// Returns (game, amoeba_idx).
fn setup() -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bosses::register(&mut g.world);

    // Player (slot 0).
    let p = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.shape = 2;
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
    }
    assert_eq!(p, 0);

    // Amoeba, armed with the registered init (mimics the mother spawn).
    let a = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[a as usize]);
    let init = g.world.istrats[bosses::IS_AMOEBA].expect("IS_AMOEBA registered");
    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 300;
        al.worldy = -200;
        al.worldz = 4000;
        al.stratptr = Some(init);
    }
    (g, a)
}

/// Manually re-park ASF_COLLIDE the way `coldet_run` clears it each frame,
/// so a colldisable stuck amoeba runs its stick tick (not the collstrat).
fn clear_collide(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
}

// ============================================================
// 1. init + drift
// ============================================================
#[test]
fn amoeba_init_promotes_to_sprite_and_drifts_in_z() {
    let (mut g, a) = setup();
    g.run_strategies(); // do_strat -> amoeba_Istrat (+ first amoeba_cont)

    let al = g.objs.aliens[a as usize];
    // s_sprite_obj x,#0 (STRATLIB.INC:873).
    assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(al.depthoffset, 0);
    assert_eq!(al.tx, 0);
    // s_set_aldata x,#hardHP,#0 — indestructible, no contact AP.
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, 0);
    // s_set_alvar B,x,al_roty,#deg180.
    assert_eq!(al.roty, DEG180);
    // s_set_alptrs …,amoebacol_Istrat,0 — collstrat wired, NO expstrat.
    assert!(al.collstratptr.is_some(), "collstrat wired");
    assert!(al.expstratptr.is_none(), "no death/explode strat");
    // amoeba_cont ran once on the fall-through: worldz -= 60 (pviewvelz 0).
    assert_eq!(al.worldz, 4000 - 60);

    // Subsequent ticks: worldz -= 60 each, plus the s_add_playerZ scroll.
    g.vars.pviewvelz = 5;
    g.run_strategies();
    assert_eq!(g.objs.aliens[a as usize].worldz, 4000 - 60 - 60 + 5);
}

// ============================================================
// 2. amoebacol -> home (shot / hit something that is not the ship)
// ============================================================
#[test]
fn amoeba_shot_enters_home_and_chases_player() {
    let (mut g, a) = setup();
    g.run_strategies(); // init

    // A non-player object it "collided" with (collobjptr != playpt).
    let dummy = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[dummy as usize]);
    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 1000; // far right of the ship at x=0
        al.sflags |= ASF_COLLIDE;
        al.collobjptr = dummy;
    }
    g.run_strategies(); // do_strat sees COLLIDE -> amoebacol -> amoebahome

    // s_jmp_alvarNE collobjptr,playpt,.end -> amoebahome (no stick).
    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 0, "not stuck");
    assert!(
        g.objs.aliens[a as usize].sflags2 & ASF2_COLLDISABLE == 0,
        "home mode keeps collisions enabled"
    );
    // amoebahome_strat chased worldx toward the player (achase rate 4, toward
    // px=0): d=-1000, min=16 (no clamp), x += (-1000>>4) = -63 -> 937.
    assert_eq!(g.objs.aliens[a as usize].worldx, 937);

    // It stays in home mode and keeps closing on the ship next tick.
    clear_collide(&mut g, a);
    let x0 = g.objs.aliens[a as usize].worldx;
    g.run_strategies();
    assert!(g.objs.aliens[a as usize].worldx < x0, "still homing");
}

// ============================================================
// 3. amoebacol -> stick (contact with the ship, room on the hull)
// ============================================================
#[test]
fn amoeba_sticks_to_player_on_contact() {
    let (mut g, a) = setup();
    g.run_strategies(); // init
    let px = g.vars.player_posx;
    let py = g.vars.player_posy;

    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 320; // contact delta captured as (delta>>2)
        al.worldy = -180;
        al.sflags |= ASF_COLLIDE;
        al.collobjptr = 0; // == playpt
    }
    g.run_strategies(); // amoebacol -> stick + first stick tick

    let al = g.objs.aliens[a as usize];
    // s_inc_var slimecount (0 -> 1).
    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 1);
    // s_playerfire off / s_set_alsflag colldisable / shape -> amoeba1.
    assert!(g.vars.pshipflags & PSF_NOFIRE != 0, "player fire disabled");
    assert!(al.sflags2 & ASF2_COLLDISABLE != 0, "colldisable set");
    assert_eq!(al.shape, SH_AMOEBA1);
    // sword1/2 = (world - player_pos) asra asra = >>2 (arithmetic).
    assert_eq!(al.sword1, (320i16 - px) >> 2);
    assert_eq!(al.sword2, (-180i16 - py) >> 2);
}

// ============================================================
// 4. stuck amoeba tracks the ship and hurts it on the 16-frame gate
// ============================================================
#[test]
fn amoeba_stick_tracks_ship_and_damages_on_gate() {
    // Full player + pcboxes so pcboxobj_B (body box) exists to be decremented.
    let mut g = Game::new();
    g.vars.game_mode = sf_game::vars::SPACE_MODE;
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.set_sv_i16(sv::MINPMOVEX, -240);
    g.vars.set_sv_i16(sv::MAXPMOVEX, 240);
    g.vars.set_sv_i16(sv::MAXPMOVEY, -20);
    g.vars.set_sv_u8(sv::LIVES, 3);
    bosses::register(&mut g.world);
    table::register_all(&mut g); // wires the player strat block used by pcboxes

    let p = strat_spawn_player(&mut g).expect("player");
    {
        let al = &mut g.objs.aliens[p as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.rotz = 0;
        al.stratptr = None; // don't run the heavy player move strat
    }
    assert!(pcbox_attach(&mut g, p), "pcbox_attach");
    let body = g.coldet.pcbox.body.unwrap();
    let body_hp0 = g.objs.aliens[body as usize].hp; // 40

    // Amoeba armed + established as stuck.
    let a = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[a as usize]);
    let init = g.world.istrats[bosses::IS_AMOEBA].unwrap();
    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 20;
        al.worldy = 8;
        al.worldz = 3000;
        al.stratptr = Some(init);
    }
    g.run_strategies(); // init
    {
        let al = &mut g.objs.aliens[a as usize];
        al.sflags |= ASF_COLLIDE;
        al.collobjptr = p;
    }
    g.run_strategies(); // stick established
    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 1);
    clear_collide(&mut g, a);

    // Move the ship; the stuck amoeba re-pins to it using the ROM's full
    // rotate_8 chain. Even at zero angles COSTAB[0]=127 attenuates the offset,
    // so this is not an exact ship + (dx,dy) copy.
    let (dx, dy) = (
        g.objs.aliens[a as usize].sword1,
        g.objs.aliens[a as usize].sword2,
    );
    g.objs.aliens[p as usize].worldx = 111;
    g.objs.aliens[p as usize].worldy = 77;

    // Land the next stick tick on gameframe & 15 == 0 (the .ndam gate) so the
    // body box takes 1 point (s_beqdec_alvar pcboxobj_B,al_hp).
    g.vars.gameframe = 15; // run_strategies -> 16
    g.run_strategies();
    let al = g.objs.aliens[a as usize];
    let (rx, ry, _) = strat_roffs_full_i16(0, 0, 0, dx, dy, 0);
    assert_eq!(al.worldx, 111 + rx, "re-pinned x to the moved ship");
    assert_eq!(al.worldy, 77 + ry, "re-pinned y to the moved ship");
    assert_eq!(
        g.objs.aliens[body as usize].hp,
        body_hp0 - 1,
        "player body box loses 1 HP on the 16-frame damage gate"
    );
}

// ============================================================
// 5. barrel roll flings the amoeba off (amoebago)
// ============================================================
#[test]
fn amoeba_stick_detaches_on_barrel_roll() {
    let (mut g, a) = setup();
    g.run_strategies(); // init
    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 10;
        al.worldy = 4;
        al.sflags |= ASF_COLLIDE;
        al.collobjptr = 0;
    }
    g.run_strategies(); // stick established
    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 1);
    assert!(g.vars.pshipflags & PSF_NOFIRE != 0);
    clear_collide(&mut g, a);

    // Player starts rolling (L/R): s_jmp_varNOTZERO player_rollZvel checked on
    // (gameframe + al1pt)&3 == 0. Within any 4 consecutive frames the gate
    // fires once -> amoebago_init.
    g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, 32);
    for _ in 0..4 {
        g.run_strategies();
        if wm8(&g, AMOEBA_SLIMECOUNT) == 0 {
            break;
        }
    }

    // amoebago_init: slimecount cleared, player fire re-enabled, back to
    // amoeba_strat. (ROM keeps colldisable -> the flung blob drifts away.)
    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 0, "stuck count reset");
    assert!(
        g.vars.pshipflags & PSF_NOFIRE == 0,
        "player fire re-enabled"
    );
    // It now drifts as a plain amoeba again (worldz decreasing).
    let z0 = g.objs.aliens[a as usize].worldz;
    g.run_strategies();
    assert!(
        g.objs.aliens[a as usize].worldz < z0,
        "flung blob drifts off"
    );
}

// ============================================================
// 6. a full hull (slimecount == 3) refuses new sticks
// ============================================================
#[test]
fn amoeba_does_not_stick_when_three_already_clinging() {
    let (mut g, a) = setup();
    g.run_strategies(); // init
    wm8_set(&mut g, AMOEBA_SLIMECOUNT, 3); // hull already full

    {
        let al = &mut g.objs.aliens[a as usize];
        al.worldx = 500;
        al.sflags |= ASF_COLLIDE;
        al.collobjptr = 0; // hit the ship, but no room
    }
    g.run_strategies(); // amoebacol: slimecount==3 -> .end -> home

    assert_eq!(wm8(&g, AMOEBA_SLIMECOUNT), 3, "count unchanged");
    assert!(
        g.objs.aliens[a as usize].sflags2 & ASF2_COLLDISABLE == 0,
        "went home, not stuck"
    );
    assert!(
        g.vars.pshipflags & PSF_NOFIRE == 0,
        "fire not disabled (never stuck)"
    );
    // Home chase moved it toward the ship (x=0).
    assert!(g.objs.aliens[a as usize].worldx < 500);
}

// ============================================================
// 7. map wiring: istrats[127] + the mother's synthetic 0x02:007f resolve
// ============================================================
#[test]
fn amoeba_registers_and_resolves_mother_synth_address() {
    let mut g = Game::new();
    table::register_all(&mut g);

    // ISTRATS.ASM row 127 is populated.
    let via_index = g.world.istrats[bosses::IS_AMOEBA];
    assert!(via_index.is_some(), "istrats[127] wired");
    assert_eq!(bosses::IS_AMOEBA, 127);

    // sf-map mothers.rs spawns the swarm as IS_SYNTH|127 == 0x02007f; the
    // table.rs address-map loop must resolve that to the same strategy.
    let via_synth = g.world.find_strategy_address(0x02007f);
    assert!(
        via_synth.is_some(),
        "mother synth address 0x02007f resolves"
    );
    assert_eq!(via_synth, via_index, "synth and index resolve identically");
}

// ---- tiny WRAM helpers ----
fn wm8(g: &Game, addr: u16) -> u8 {
    g.vars.read_ext8(addr)
}
fn wm8_set(g: &mut Game, addr: u16, v: u8) {
    g.vars.write_ext8(addr, v);
}
