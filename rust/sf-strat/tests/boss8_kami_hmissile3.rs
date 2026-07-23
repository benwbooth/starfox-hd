//! Tick 144: boss8 nucleus launcher → `fire_kamiHmissile1` / `hmissile3`
//! (GASTRATS.ASM:103-110), replacing the simplified boss8_kamimissile_strat.

use sf_game::alien::{ASF3_REALOBJ, NUMBER_AL};
use sf_game::Game;
use sf_strat::bosses::b8_fire_kamimissile;
use sf_strat::enemy_a::{hmissile3_istrat, hmissile3_strat};

const ASF2_SFLAG1: u8 = 0x10;
const ASF2_SFLAG2: u8 = 0x20;
const SH_ZACO_9: u16 = 269;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.worldx = 0;
    al.worldy = -40;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posz = z;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// Launcher fire uses weapon-lane hmissile3 + zaco_9 shape + sflag1 + al_ptr.
#[test]
fn b8_kami_uses_hmissile3_and_sflag1() {
    let mut g = Game::new();
    spawn_player(&mut g, 500);
    let launch = spawn(&mut g);
    g.objs.aliens[launch as usize].worldz = 1000;
    g.objs.aliens[launch as usize].roty = 40; // must be restored

    let shot = b8_fire_kamimissile(&mut g, launch, 0).expect("kami");
    assert_eq!(
        g.objs.aliens[launch as usize].roty, 40,
        "firer roty restored"
    );

    let al = &g.objs.aliens[shot as usize];
    assert_eq!(al.shape, SH_ZACO_9);
    assert_eq!(al.hp, 2);
    assert_eq!(al.ap, 8);
    assert_eq!(al.vel, 40);
    assert_eq!(al.count, 100);
    assert_eq!(al.ptr, 1, "al_ptr = playpt (index+1)");
    assert!(al.sflags2 & ASF2_SFLAG1 != 0, "sflag1 skips missbound");
    assert!(al.stratptr.is_some());
    let shot_strat = al.stratptr;

    // Same Game registry: probe hmissile3_istrat must share stratptr.
    let probe = spawn(&mut g);
    hmissile3_istrat(&mut g, probe);
    assert_eq!(
        shot_strat, g.objs.aliens[probe as usize].stratptr,
        "stratptr == hmissile3_strat"
    );
}

/// hmissile3 laser weave: |dz| in [1000,2000) on notdelay-3 fires twin lasers.
#[test]
fn hmissile3_fires_twin_lasers_in_dz_band() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let miss = spawn(&mut g);
    g.objs.aliens[miss as usize].worldz = 1500; // |dz|=1500
    g.objs.aliens[miss as usize].worldx = 0;
    g.objs.aliens[miss as usize].worldy = -40;
    g.objs.aliens[miss as usize].ptr = 1; // player
    g.objs.aliens[miss as usize].vel = 40;
    g.objs.aliens[miss as usize].count = 100;
    g.vars.gameframe = 0; // notdelay-3: (gf & 7) == 0 → fire
    hmissile3_istrat(&mut g, miss);

    let before = (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count();
    hmissile3_strat(&mut g, miss);
    let after = (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count();
    assert!(
        after >= before + 2,
        "twin RELSLOWELASER: before={before} after={after}"
    );
    assert!(g.objs.aliens[miss as usize].sflags2 & ASF2_SFLAG2 == 0);
}

/// Near target (|dz|<500) latches sflag2 and stops aiming.
#[test]
fn hmissile3_latches_sflag2_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let miss = spawn(&mut g);
    g.objs.aliens[miss as usize].worldz = 200; // |dz|=200 < 500
    g.objs.aliens[miss as usize].ptr = 1;
    g.objs.aliens[miss as usize].vel = 40;
    g.objs.aliens[miss as usize].count = 50;
    hmissile3_istrat(&mut g, miss);
    hmissile3_strat(&mut g, miss);
    assert!(g.objs.aliens[miss as usize].sflags2 & ASF2_SFLAG2 != 0);
}
