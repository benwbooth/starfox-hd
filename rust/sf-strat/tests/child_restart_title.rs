//! Tick 119: SETOBJTOBECHILD* + SET_RESTART_POSITION + title/planets/fade
//! + sprouty.withdraw_i + Mario show/dot stand-ins.

use sf_game::alien::{ASF4_CHILDOBJ, ASF4_MOTHEROBJ};
use sf_game::debug_draw::{BootInit, DisplayFx, HdmaRegion, MarioDraw};
use sf_game::windows::Windows;
use sf_game::Game;
use sf_strat::bosses::sprouty_withdraw_init;
use sf_strat::common::{set_obj_to_be_child_xy, set_obj_to_be_child_yx};

#[test]
fn set_obj_to_be_child_walks_sword1_chain() {
    let mut g = Game::new();
    let mother = g.objs.alloc().expect("mother");
    let c1 = g.objs.alloc().expect("c1");
    let c2 = g.objs.alloc().expect("c2");
    {
        let m = &mut g.objs.aliens[mother as usize];
        m.sflags4 |= ASF4_MOTHEROBJ;
        m.sword1 = (c1 as i16).wrapping_add(1);
    }
    {
        let a = &mut g.objs.aliens[c1 as usize];
        a.sflags4 |= ASF4_CHILDOBJ;
        a.sbyte1 = 1;
        a.sword1 = (c2 as i16).wrapping_add(1);
        a.ptr = (mother as u16).wrapping_add(1);
    }
    {
        let a = &mut g.objs.aliens[c2 as usize];
        a.sflags4 |= ASF4_CHILDOBJ;
        a.sbyte1 = 3;
        a.sword1 = 0;
        a.ptr = (mother as u16).wrapping_add(1);
    }

    assert_eq!(set_obj_to_be_child_yx(&g, mother, 1), Some(c1));
    assert_eq!(set_obj_to_be_child_xy(&g, mother, 3), Some(c2));
    assert_eq!(set_obj_to_be_child_yx(&g, mother, 9), None);
}

#[test]
fn set_restart_position_snapshots_map_stacks() {
    let mut g = Game::new();
    g.vars.mapptr = 0x1234;
    g.vars.currentbg = 7;
    g.world.jsr_stack[0] = 0xAAAA;
    g.world.jsr_top = 1;
    g.world.num_jsr = 1;
    g.world.loop_addrs[0] = 0xBBBB;
    g.world.loop_counts[0] = 3;
    g.world.num_loops = 1;

    g.world
        .set_restart_position(g.vars.mapptr, g.vars.currentbg, 30);
    assert_eq!(g.world.restart.mapptr, 0x1234);
    assert_eq!(g.world.restart.bg, 7);
    assert_eq!(g.world.restart.palfade, 30);
    assert_eq!(g.world.restart.jsr_top, 1);
    assert_eq!(g.world.restart.num_loops, 1);

    // Mutate live stacks, then restore.
    g.world.jsr_top = 0;
    g.world.num_loops = 0;
    g.vars.mapptr = 0;
    let (bg, fade) = g.world.apply_restart(&mut g.vars.mapptr);
    assert_eq!(g.vars.mapptr, 0x1234);
    assert_eq!(bg, 7);
    assert_eq!(fade, 30);
    assert_eq!(g.world.jsr_top, 1);
    assert_eq!(g.world.num_loops, 1);
    assert_eq!(g.world.jsr_stack[0], 0xAAAA);
}

#[test]
fn title_planets_fade_and_mario_show() {
    let mut b = BootInit::default();
    b.setup_planets();
    b.title_seq();
    assert!(b.setup_planets >= 1);
    assert!(b.title_seq >= 1);
    assert!(b.init_game >= 1);
    assert!(b.init_3d >= 1);

    let mut d = DisplayFx::default();
    let mut w = Windows::default();
    d.exit_spec_do_fade_down(&mut w);
    assert_eq!(d.exit_fade_down, 1);
    assert!(w.is_map_fade_active());

    let mut m = MarioDraw::default();
    m.mshow_grid();
    m.mshow_dust();
    m.mgr_draw_dot();
    m.mshow_obj_exit_not_drawn();
    assert_eq!(m.show_grid, 1);
    assert_eq!(m.show_dust, 1);
    assert_eq!(m.draw_dot, 1);
    assert_eq!(m.show_obj_exit_not_drawn, 1);

    let h = HdmaRegion::default();
    assert!(h.oops_table);
}

#[test]
fn sprouty_withdraw_i_public() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].animframe = 0x80 | 8;
    sprouty_withdraw_init(&mut g, idx);
    // Frame shrinks by 4 on first tick of withdraw strat.
    let frame = g.objs.aliens[idx as usize].animframe & 0x7f;
    assert_eq!(frame, 4);
}
