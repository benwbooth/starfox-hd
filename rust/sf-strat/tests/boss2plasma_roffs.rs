//! Tick 176: boss2plasma orbit uses non-uniform Roffs scales 2,0,4.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::bosses::{boss2petal_strat, boss2plasma_strat, strat_boss2_init};
use sf_strat::enemy_a::wm;
use sf_strat::snes_trig::strat_roffs_yaw_scaled_xyz;

const BOSS2_SFLAG3: u8 = 0x40;
const BOSS2PETAL_AP: u8 = 1;
const BOSS2PLASMA_AP: u8 = 8;

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

#[test]
fn boss2plasma_orbit_applies_scales_2_0_4() {
    let mut g = Game::new();
    spawn_player(&mut g);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = g.objs.alloc().expect("boss");
    g.objs.aliens[boss as usize].active = true;
    strat_boss2_init(&mut g, boss);

    let petal = (0..g.objs.aliens.len())
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].ap == BOSS2PETAL_AP)
        .expect("petal") as u16;

    g.objs.aliens[boss as usize].sflags2 |= BOSS2_SFLAG3;
    g.objs.aliens[boss as usize].worldx = 100;
    g.objs.aliens[boss as usize].worldy = -40;
    g.objs.aliens[boss as usize].worldz = 500;
    boss2petal_strat(&mut g, petal);

    let plasma = (0..g.objs.aliens.len())
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].ap == BOSS2PLASMA_AP)
        .expect("plasma") as u16;

    // Freeze petal pose as the orbit base (petal_strat would re-copy mother).
    g.objs.aliens[petal as usize].worldx = 100;
    g.objs.aliens[petal as usize].worldy = -40;
    g.objs.aliens[petal as usize].worldz = 500;
    g.objs.aliens[plasma as usize].sbyte1 = 64;
    g.objs.aliens[plasma as usize].sbyte2 = 10;
    g.objs.aliens[plasma as usize].sword2 = -40; // worldy snap target
    boss2plasma_strat(&mut g, plasma);

    let (rx, _ry, rz) = strat_roffs_yaw_scaled_xyz(64, 0, 0, 10, 2, 0, 4);
    let al = &g.objs.aliens[plasma as usize];
    // worldy is overwritten to sword2 after Roffs (ROM hover chase).
    assert_eq!(al.worldx, 100i16.wrapping_add(rx));
    assert_eq!(al.worldy, -40);
    assert_eq!(al.worldz, 500i16.wrapping_add(rz));
    assert_ne!(rx, 0, "yaw 90° must put radius into X before <<2");
}
