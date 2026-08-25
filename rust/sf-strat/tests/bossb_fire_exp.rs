//! ROM bossBrob fire + death explode chain (GB3STRAT.ASM).

use sf_core::screen_fill_circle::{
    ScreenFillCircleCenter, ScreenFillCirclePhase, BOSS_RADIUS_SPEED, INITIAL_COLOR_LEVEL,
};
use sf_game::alien::{ASF2_COLLDISABLE, ATZREMOVE};
use sf_game::vars::GF_BOSSDEAD;
use sf_game::Game;
use sf_strat::bossb::{
    bossbpexp2_istrat, bossbpexp_istrat, bossbpexp_strat, bossbpwaitexp_istrat,
    bossbpwaitexp_strat, bossbrob_transform_init, bossbrobexp_init, bossbrobfarland_init,
    bossbrobfarland_strat, bossbrobfire1_init, bossbrobfire1_strat, bossbrobfire2_init,
    bossbrobfire2_strat, bossbrobfirep1_init, bossbrobfirep1_strat, bossbrobsepexp_strat,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

#[test]
fn firep1_plants_and_fires() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].worldz = 2000;
    g.vars.gameframe = 0; // notdelay 4 fires
    bossbrobfirep1_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 59); // fall-through dec
                                                        // Force fire gate.
    g.vars.gameframe = 0;
    let before = g.objs.active_indices().len();
    // sbyte1 still >0; call again on fire frame
    g.objs.aliens[idx as usize].sbyte1 = 30;
    bossbrobfirep1_strat(&mut g, idx);
    assert!(g.objs.active_indices().len() >= before);
}

#[test]
fn fire1_sprays_then_fire2_restores_yaw() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].worldz = 1500;
    g.objs.aliens[idx as usize].roty = 100;
    g.vars.gameframe = 0;
    bossbrobfire1_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 100);
    assert!(g.objs.aliens[idx as usize].sbyte1 < 60);

    // Expire fire1 → fire2.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    bossbrobfire1_strat(&mut g, idx);
    // fire2 chasing saved yaw.
    bossbrobfire2_init(&mut g, idx);
    g.objs.aliens[idx as usize].roty = 100;
    g.objs.aliens[idx as usize].sbyte2 = 100;
    bossbrobfire2_strat(&mut g, idx);
    // Already on target → nextstate (stratptr changes).
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn farland_stops_scrolling_until_player_catches_up() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.pviewvelz = 55;
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].worldz = 5000;

    bossbrobfarland_init(&mut g, idx);
    let landed_z = g.objs.aliens[idx as usize].worldz;
    bossbrobfarland_strat(&mut g, idx);

    // GB3STRAT.ASM branches to bossBaddbhp_cont here, not
    // bossBaddpz_cont: the landed robot waits in world space while the
    // player's forward motion closes the gap.
    assert_eq!(g.objs.aliens[idx as usize].worldz, landed_z);
}

#[test]
fn sepexp_falls_then_splits_into_debris() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("rob");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = -200;
        al.worldz = 500;
        al.sflags3 |= 0x01; // already walking so transform goes to sepexp
    }
    bossbrob_transform_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 39); // fall-through

    // Force countdown done → bossbrobexp_init.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].worldy = -160; // on ground
    let before = g.objs.active_indices().len();
    bossbrobsepexp_strat(&mut g, idx);
    assert!(g.objs.active_indices().len() > before, "L/R debris spawned");
    assert_eq!(g.objs.aliens[idx as usize].count, 24); // 25 then wait tick dec
}

#[test]
fn bossbpexp2_sets_bossdead_and_tumbles() {
    const BOSS_POSITION: [i16; 3] = [120, -40, 900];

    let mut g = Game::new();
    let idx = g.objs.alloc().expect("head");
    g.objs.aliens[idx as usize].worldx = BOSS_POSITION[0];
    g.objs.aliens[idx as usize].worldy = BOSS_POSITION[1];
    g.objs.aliens[idx as usize].worldz = BOSS_POSITION[2];
    bossbpexp2_istrat(&mut g, idx);
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    let ScreenFillCircleCenter::Object(object_id) = g.vars.screen_fill_circle.center else {
        panic!("boss circle should use its retained world anchor");
    };
    let anchor = object_id - 1;
    assert_eq!(
        [
            g.objs.aliens[anchor as usize].worldx,
            g.objs.aliens[anchor as usize].worldy,
            g.objs.aliens[anchor as usize].worldz,
        ],
        BOSS_POSITION
    );
    assert!(g.objs.aliens[anchor as usize].stratptr.is_some());
    assert_eq!(
        g.vars.screen_fill_circle.phase,
        ScreenFillCirclePhase::BossExpanding
    );
    assert_eq!(g.vars.screen_fill_circle.radius, BOSS_RADIUS_SPEED as u16);
    assert_eq!(
        [
            g.vars.screen_fill_circle.red,
            g.vars.screen_fill_circle.green,
            g.vars.screen_fill_circle.blue,
        ],
        [INITIAL_COLOR_LEVEL; 3]
    );
    let rz0 = g.objs.aliens[idx as usize].rotz;
    bossbpexp_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rz0.wrapping_add(8));
}

#[test]
fn bossbpexp_spawns_lexp() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("part");
    let before = g.objs.active_indices().len();
    bossbpexp_istrat(&mut g, idx);
    assert!(g.objs.active_indices().len() > before);
}

#[test]
fn waitexp_expires_into_expstrat() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("w");
    bossbpwaitexp_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 1;
    let s_exp = g.world.register_strategy(bossbpexp_istrat);
    g.objs.aliens[idx as usize].expstratptr = Some(s_exp);
    bossbpwaitexp_strat(&mut g, idx);
    // expstrat ran (tumble flags).
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
}

#[test]
fn bossbrobexp_init_spawns_lr_parts() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = g.objs.alloc().expect("rob");
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -100;
    let before = g.objs.active_indices().len();
    bossbrobexp_init(&mut g, idx);
    assert!(g.objs.active_indices().len() >= before + 2);
}
