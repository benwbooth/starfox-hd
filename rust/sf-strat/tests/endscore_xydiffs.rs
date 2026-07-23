//! Tick 108: XYDIFFS + MAKEENDOBJ/MAKENUM + CLEARHVOFS/GAMECLIPWINDOW + CHECKIFIAMEND.

use sf_game::clip::{BgScrollOffsets, GameClipWindow};
use sf_game::Game;
use sf_strat::common::{xy_diffs, xy_diffs_abs};
use sf_strat::endscore::{
    check_if_i_am_end, makeendobj, makeendobj2, makeendobjn, makenumt, makenumt_msg, END_OBJ2_Z,
    END_OBJ_COLOUR, END_OBJ_COLOUR2, END_OBJ_Z, MSG_DIGIT_TAG, SH_ZACO_4,
};

fn spawn_player(g: &mut Game) -> u16 {
    // Obj_GetPlayer is slot 0 when active — allocate until we own slot 0,
    // or force-activate aliens[0] after a normal alloc if free-list order varies.
    let idx = g.objs.alloc().expect("player");
    if idx != 0 {
        // Move state into slot 0 for player() lookups.
        g.objs.aliens[0] = g.objs.aliens[idx as usize].clone();
        g.objs.aliens[0].active = true;
    }
    g.objs.aliens[0].worldx = 100;
    g.objs.aliens[0].worldy = 200;
    g.objs.aliens[0].worldz = 300;
    g.vars.internal_playpt = 0;
    0
}

#[test]
fn xy_diffs_abs_is_manhattan() {
    // ROM xydiffs_abs_l: |dx|+|dy| (NOT scaled-Euclidean).
    assert_eq!(xy_diffs_abs(0, 0, 0, 0), 0);
    assert_eq!(xy_diffs_abs(0, 0, 400, 0), 400);
    assert_eq!(xy_diffs_abs(0, 0, 0, 300), 300);
    assert_eq!(xy_diffs_abs(10, 20, 40, 50), 60); // |30|+|30|
    assert_eq!(xy_diffs_abs(40, 50, 10, 20), 60);
}

#[test]
fn xy_diffs_obj_pair() {
    let mut g = Game::new();
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    g.objs.aliens[a as usize].worldx = 0;
    g.objs.aliens[a as usize].worldy = 0;
    g.objs.aliens[b as usize].worldx = 100;
    g.objs.aliens[b as usize].worldy = 50;
    assert_eq!(
        xy_diffs(&g.objs.aliens[a as usize], &g.objs.aliens[b as usize]),
        150
    );
}

#[test]
fn makeendobj_spawns_at_player_plus_offset() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = makeendobj(&mut g, 0x1234, -1600, 0).expect("spawn");
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.shape, SH_ZACO_4);
    assert_eq!(al.worldx, 100i16.wrapping_add(-1600));
    assert_eq!(al.worldy, 200);
    assert_eq!(al.worldz, 300i16.wrapping_add(END_OBJ_Z));
    assert_eq!(al.depthoffset, END_OBJ_COLOUR as i16);
    assert_eq!(al.coltab, 0x1234);
}

#[test]
fn makeendobj2_and_digit_variants() {
    let mut g = Game::new();
    spawn_player(&mut g);
    assert_eq!(makenumt_msg(5), MSG_DIGIT_TAG | 10);
    let idx = makeendobjn(&mut g, 3, 0, -1400).expect("n");
    assert_eq!(g.objs.aliens[idx as usize].coltab, makenumt_msg(3));
    let idx2 = makeendobj2(&mut g, 0xABCD, 500, 750).expect("2");
    assert_eq!(
        g.objs.aliens[idx2 as usize].depthoffset,
        END_OBJ_COLOUR2 as i16
    );
    assert_eq!(
        g.objs.aliens[idx2 as usize].worldz,
        300i16.wrapping_add(END_OBJ2_Z)
    );
}

#[test]
fn makenumt_advances_cla1() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let (idx, next) = makenumt(&mut g, 7, -200);
    assert!(idx.is_some());
    assert_eq!(next, -50); // -200 + 150
    let al = &g.objs.aliens[idx.unwrap() as usize];
    assert_eq!(al.worldx, -100); // 100 + (-200)
    assert_eq!(al.worldy, 1300); // 200 + 1100
    assert_eq!(al.depthoffset, 14);
}

#[test]
fn check_if_i_am_end_arms_c_type() {
    let mut c = 0u8;
    assert!(!check_if_i_am_end(3, 7, &mut c));
    assert_eq!(c, 0);
    assert!(check_if_i_am_end(7, 7, &mut c));
    assert_eq!(c, 30);
}

#[test]
fn clip_and_clear_hvofs() {
    let w = GameClipWindow::game();
    assert_eq!(w.clx2, 223);
    assert_eq!(w.vanishx, 112);
    let mut s = BgScrollOffsets {
        bg3_vofs: 99,
        ..Default::default()
    };
    s.clear_hvofs();
    assert_eq!(s.bg3_vofs, 0);
}
