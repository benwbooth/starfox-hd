//! Tick 117: FLOAT* + COUNT_SHAPES + WINGLAZER*INIT + INIT*/FNMI + MDRAW*/MMAKE.

use sf_game::debug_draw::{BootInit, MarioDraw, PART_FADE_TAB_LEN};
use sf_game::Game;
use sf_path::ids::PATH_ID_FADEINTOTAL;
use sf_strat::common::{
    count_shapes, float128_srou, float256_srou, float32_srou, float64_srou, flout_srou,
    WM_FLOATVAR1, WM_FLOATVAR2,
};
use sf_strat::enemies_ground::{winglazerman2_init, winglazerman3_init, winglazermango_init};
use sf_strat::snes_trig::SINTAB;

fn spawn(g: &mut Game, shape: u16) -> u16 {
    let i = g.objs.alloc().unwrap();
    g.objs.aliens[i as usize].shape = shape;
    i
}

#[test]
fn float_srou_adds_sintab_scaled() {
    let mut g = Game::new();
    let idx = spawn(&mut g, 1);
    g.vars.write_ext8(WM_FLOATVAR1, 0);
    g.vars.write_ext8(WM_FLOATVAR2, 64);
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = 200;

    let (x1, x2) = flout_srou(&g, idx);
    assert_eq!(x1, idx as u8); // 0 + idx
    assert_eq!(x2, 64u8.wrapping_add(idx as u8));

    // float64: scale -3 → sintab / 8
    let tpx = (SINTAB[x1 as usize] as i16) / 8;
    let tpy = (SINTAB[x2 as usize] as i16) / 8;
    float64_srou(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 100i16.wrapping_add(tpx));
    assert_eq!(g.objs.aliens[idx as usize].worldy, 200i16.wrapping_add(tpy));

    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    float256_srou(&mut g, idx);
    float128_srou(&mut g, idx);
    float32_srou(&mut g, idx);
    // Just ensure they run without panic; positions changed.
    assert!(
        g.objs.aliens[idx as usize].worldx != 0
            || g.objs.aliens[idx as usize].worldy != 0
            || SINTAB[x1 as usize] == 0
    );
}

#[test]
fn count_shapes_counts_active() {
    let mut g = Game::new();
    spawn(&mut g, 10);
    spawn(&mut g, 10);
    spawn(&mut g, 11);
    assert_eq!(count_shapes(&g, 10), 2);
    assert_eq!(count_shapes(&g, 11), 1);
    assert_eq!(count_shapes(&g, 99), 0);
}

#[test]
fn winglazerman_inits_public() {
    let mut g = Game::new();
    let idx = spawn(&mut g, 50);
    g.objs.aliens[idx as usize].sbyte2 = 2;
    winglazerman2_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);

    winglazerman3_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29); // 30 then strat decs once
                                                        // Actually 3_init sets 30 then calls strat which may dec — check >= 0
    assert!(g.objs.aliens[idx as usize].sbyte1 <= 30);

    winglazermango_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 99); // 100 then strat may dec
                                                       // count set to 100 then strat runs — may still be 100 if no dec on first line
    assert!(g.objs.aliens[idx as usize].count <= 100);
}

#[test]
fn boot_init_and_mario_draw() {
    let mut b = BootInit::default();
    b.init_game();
    b.init_game_3d();
    b.init_sprites();
    b.fnmi();
    assert_eq!(b.init_game, 1);
    assert_eq!(b.init_game_3d, 1);
    assert_eq!(b.init_screen, 1);
    assert_eq!(b.init_3d, 1);
    assert_eq!(b.init_sprites, 1);
    assert_eq!(b.fnmi, 1);

    let mut m = MarioDraw::default();
    m.mmake_particles();
    m.mwindrawline();
    m.mdodrawline();
    m.mdraw_solid_box();
    m.mdraw_uv_list();
    m.mdraw_tsphere();
    m.mdraw_sprite32();
    m.mdraw_hud();
    m.mdraw_tpoly();
    m.mdraw_horz_line();
    m.mdraw_dust();
    m.mdraw_poly();
    m.mdraw_sphere();
    assert_eq!(m.make_particles, 1);
    assert_eq!(m.sphere, 1);
    assert_eq!(PART_FADE_TAB_LEN, 16);
    assert_eq!(PATH_ID_FADEINTOTAL, 284);
}
