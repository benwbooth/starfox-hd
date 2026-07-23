//! Tick 113: FOXY_CONTINUE/TRANS + FOX_SPRITES + DRAWSOME3D + CLRONEHALF
//! + FIND_OBJECT/FINDTARGET + JUMPTOSTATE + ENDTRANS.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::charmap::{CharMap, CharMapScreen};
use sf_game::foxy::{
    foxy_continue_enter, EndTrans, FoxyContinue, FOXY_CURSOR_X_CONTINUE, FOXY_CURSOR_X_QUIT,
    FOXY_CURSOR_Y,
};
use sf_game::planets::DEFAULT_LIVES;
use sf_game::Game;
use sf_strat::common::{find_object, jump_to_state};

#[test]
fn foxy_continue_requires_credits() {
    let mut cm = CharMap::new();
    assert!(foxy_continue_enter(0, &mut cm).is_none());
    assert_eq!(cm.screen, CharMapScreen::None);

    let enter = foxy_continue_enter(2, &mut cm).expect("credits");
    assert_eq!(cm.screen, CharMapScreen::Fox);
    assert_eq!(enter.charmap, CharMapScreen::Fox);
    assert_eq!(enter.lives, DEFAULT_LIVES);
    assert_eq!(enter.dma.sprites, 1);
    // enter: 2 half clears + 2 foxytrans × 2 halves = 6
    assert_eq!(enter.foxy.half_clears, 6);
    // enter fox_sprites + 2 foxytrans
    assert_eq!(enter.foxy.sprites_built, 3);
    assert_eq!(enter.foxy.draw3d_frames, 2);
    assert_eq!(enter.foxy.cursor_x, FOXY_CURSOR_X_CONTINUE);
    assert_eq!(enter.foxy.cursor_y, FOXY_CURSOR_Y);
    assert_eq!(enter.foxy.rot_dy, 4);
}

#[test]
fn fox_sprites_cursor_follows_option() {
    let mut f = FoxyContinue::default();
    f.fox_sprites();
    assert_eq!(f.cursor_x, FOXY_CURSOR_X_CONTINUE);
    f.toggle_option();
    assert_eq!(f.cursor_x, FOXY_CURSOR_X_QUIT);
    assert!(!f.chose_continue());
    f.toggle_option();
    assert!(f.chose_continue());
}

#[test]
fn foxytrans_clears_and_draws() {
    let mut f = FoxyContinue::default();
    f.rot_dx = 1;
    f.foxy_trans();
    assert_eq!(f.half_clears, 2);
    assert_eq!(f.sprites_built, 1);
    assert_eq!(f.draw3d_frames, 1);
    assert_eq!(f.rot_x, 1);
    assert_eq!(f.rot_y, 4);
}

#[test]
fn endtrans_counts_half_clears() {
    let mut e = EndTrans::default();
    e.run();
    e.run();
    assert_eq!(e.ticks, 2);
    assert_eq!(e.half_clears, 4);
}

#[test]
fn find_object_by_shape_skips_self() {
    let mut g = Game::new();
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    let c = g.objs.alloc().unwrap();
    g.objs.aliens[a as usize].shape = 10;
    g.objs.aliens[b as usize].shape = 20;
    g.objs.aliens[c as usize].shape = 10;
    // Active list is push-front: c -> b -> a
    let mut fobj = g.objs.active_head;
    let found = find_object(&g, 10, c, &mut fobj).expect("shape 10");
    assert_eq!(found, a); // skip self c, then b≠10, then a
                          // fobj advanced to successor of a
    assert_eq!(fobj, g.objs.aliens[a as usize].next);
}

#[test]
fn find_object_any_requires_realobj() {
    let mut g = Game::new();
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    let c = g.objs.alloc().unwrap();
    g.objs.aliens[b as usize].sflags3 |= ASF3_REALOBJ;
    // list: c -> b -> a; search_from = c means get_anyobj starts at _next(c)=b
    let mut fobj = Some(c);
    let found = find_object(&g, 0, a, &mut fobj).expect("realobj");
    assert_eq!(found, b);
    assert_eq!(fobj, Some(b));
}

#[test]
fn jump_to_state_indexes_table() {
    let table: [[u8; 4]; 3] = [
        [0x09, 0x34, 0x12, 0],
        [0x0A, 0x78, 0x56, 0],
        [0x0B, 0xBC, 0x9A, 0],
    ];
    assert_eq!(jump_to_state(1, &table), Some((0x0A, 0x5678)));
    assert_eq!(jump_to_state(0, &table), Some((0x09, 0x1234)));
    assert_eq!(jump_to_state(3, &table), None);
}
