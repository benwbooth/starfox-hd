//! Tick 111: BUILD_DRAWLIST + WAITDMA + CLEARSPRITES + FADEHALF2NORM + CALCBG2VOFFSETS.

use sf_game::bgs::{calc_bg2_voffsets, Bg2VofsResult};
use sf_game::clip::WaitDma;
use sf_game::draw::build_list;
use sf_game::obj::Objects;
use sf_game::vars::{GameVars, GF_NOZREMOVE};
use sf_game::windows::{Windows, WINDOW_MODE_HALFFADE};

#[test]
fn build_drawlist_emits_shaped_aliens() {
    // ROM build_drawlist is an empty rts; live path is draw::build_list.
    let mut objs = Objects::init();
    let idx = objs.alloc().unwrap();
    objs.aliens[idx as usize].shape = 1;
    objs.aliens[idx as usize].worldx = 10;
    objs.aliens[idx as usize].worldy = 20;
    objs.aliens[idx as usize].worldz = 100;
    let mut out = Vec::new();
    build_list(&mut objs, 0, 0, 0, 0, 0, GF_NOZREMOVE, &|_| None, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].shape_id, 1);
}

#[test]
fn waitdma_is_noop_recording_line() {
    let mut w = WaitDma::default();
    w.wait(50);
    assert_eq!(w.last_line, 50);
    w.wait_224();
    assert_eq!(w.last_line, 222);
}

#[test]
fn fadehalf2norm_steps() {
    let mut w = Windows::new();
    w.start_half_fade();
    assert_eq!(w.slots[0].mode, WINDOW_MODE_HALFFADE);
    assert_eq!(w.slots[0].wm_val, 31);
    assert!(w.fade_half_to_norm());
    assert_eq!(w.slots[0].wm_val, 30);
}

#[test]
fn calcbg2voffsets_skip_when_dovofs_off() {
    let mut vars = GameVars::init();
    assert_eq!(
        calc_bg2_voffsets(&mut vars, 0x100),
        Bg2VofsResult::default()
    );
    vars.dovofs = 1;
    vars.shared.do_depth_rotation = 1;
    // Seed lastrot to a different key so the first rebuild fires.
    vars.shared.last_rotation = u16::MAX;
    let r = calc_bg2_voffsets(&mut vars, 0x0500); // hi=5 → key=10
    assert!(r.needs_dma);
    assert_eq!(r.table_key, 10);
    assert_eq!(vars.shared.last_rotation, 10);
    // Same key → skip.
    assert_eq!(
        calc_bg2_voffsets(&mut vars, 0x0500),
        Bg2VofsResult::default()
    );
}
