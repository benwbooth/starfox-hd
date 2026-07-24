//! Tick 80: ship1/ship1a/ship1col + ship3b/c/cont + boss2rots/doboss* leaves.

use sf_core::player_view::PlayerViewMode;
use sf_game::alien::{ASF_COLLDISABLE, ASF_COLLIDE, ASF_NOHITAFFECT, ATGND};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::bosses::{boss2rots_srou, doboss2rot_srou, dobossrot_srou, dobossrotx4_srou};
use sf_strat::common::StratRam;
use sf_strat::enemy_a::{
    boss_attach_child_to_mother, ship1_istrat, ship1_strat, ship1a_cont, ship1a_istrat,
    ship1a_strat, ship1col_istrat, ship3_cont, ship3_istrat, ship3_strat, ship3a_strat,
    ship3b_strat, ship3c_init, ship3c_strat, DEG180, DEG90,
};
use sf_strat::player::{player_sv as sv, set_player_out_of_cock, COCKPIT_EXIT_FRAMES};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 100;
    idx
}

fn register_cockpit_exit(g: &mut Game) {
    let transition = g.world.register_strategy(set_player_out_of_cock);
    g.vars.strategy_bindings.leave_cockpit = Some(transition.0);
}

#[test]
fn ship1_ship1a_col() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let s = spawn_obj(&mut g);
    g.objs.aliens[s as usize].sbyte1 = 2; // turn right
    ship1_istrat(&mut g, s);
    assert_eq!(g.objs.aliens[s as usize].hp, 20);
    assert_eq!(g.objs.aliens[s as usize].vel, 10);
    assert_eq!(g.objs.aliens[s as usize].count, 100);
    let z0 = g.objs.aliens[s as usize].worldz;
    ship1_strat(&mut g, s);
    // far from player → speed toward 60, life--, pitch/yaw nudge
    assert_eq!(g.objs.aliens[s as usize].count, 99);
    assert!(g.objs.aliens[s as usize].vel >= 10);
    assert_ne!(g.objs.aliens[s as usize].worldz, z0); // moved via vecs+playerZ

    let a = spawn_obj(&mut g);
    g.objs.aliens[a as usize].vel = 20;
    g.objs.aliens[a as usize].roty = 0;
    ship1a_istrat(&mut g, a);
    assert_eq!(g.objs.aliens[a as usize].hp, 20);
    let y0 = g.objs.aliens[a as usize].worldy;
    // player behind ship (player z=0, ship z=3000) → fire path may run
    g.vars.gameframe = 0; // delay-2 open
    ship1a_strat(&mut g, a);
    assert_eq!(g.objs.aliens[a as usize].worldy, y0.wrapping_sub(1));
    ship1a_cont(&mut g, a);

    // HF2 col → hitflash path clears hitflags
    let c = spawn_obj(&mut g);
    let partner = spawn_obj(&mut g);
    g.objs.aliens[c as usize].hitflags = 0x02; // HF2
    g.objs.aliens[c as usize].collobjptr = partner;
    g.objs.aliens[c as usize].sflags |= ASF_COLLIDE;
    ship1col_istrat(&mut g, c);
    assert_eq!(g.objs.aliens[c as usize].hitflags, 0);

    // non-HF2 → clear collide, resume
    let c2 = spawn_obj(&mut g);
    g.objs.aliens[c2 as usize].hitflags = 0;
    g.objs.aliens[c2 as usize].sflags |= ASF_COLLIDE;
    g.objs.aliens[c2 as usize].stratptr = g.objs.aliens[s as usize].stratptr;
    ship1col_istrat(&mut g, c2);
    assert_eq!(g.objs.aliens[c2 as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn ship3_b_c_cont_and_boss2rots() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    register_cockpit_exit(&mut g);

    let s3 = spawn_obj(&mut g);
    ship3_istrat(&mut g, s3);
    assert_eq!(g.objs.aliens[s3 as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[s3 as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[s3 as usize].type_ & ATGND, 0);
    assert_eq!(g.objs.aliens[s3 as usize].rotz, DEG90);
    assert_eq!(g.objs.aliens[s3 as usize].vy, -40);
    ship3_strat(&mut g, s3);

    g.vars.player_view_mode = PlayerViewMode::Cockpit;
    g.objs.aliens[s3 as usize].worldz = 500;
    g.objs.aliens[0].worldz = 0;
    ship3a_strat(&mut g, s3);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::LeavingCockpit);
    assert_eq!(
        g.vars.sv_u8(sv::PSVAR_BYTE1),
        COCKPIT_EXIT_FRAMES,
        "ship3 approach must start the authored cockpit exit"
    );

    // ship3b locks vz = -pviewvelz
    g.vars.pviewvelz = 65;
    let b = spawn_obj(&mut g);
    ship3b_strat(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].vz, (-65i16) as i16);

    let c = spawn_obj(&mut g);
    ship3c_init(&mut g, c);
    assert_eq!(g.objs.aliens[c as usize].vy, -10);
    assert_eq!(g.objs.aliens[c as usize].vz, 30);
    let z0 = g.objs.aliens[c as usize].worldz;
    ship3c_strat(&mut g, c);
    assert_ne!(g.objs.aliens[c as usize].worldz, z0);

    // cont with player HP0 → ship3c
    let d = spawn_obj(&mut g);
    g.vars.pshipflags2 |= sf_game::vars::PSF2_PLAYERHP0;
    ship3_cont(&mut g, d);
    assert_eq!(g.objs.aliens[d as usize].vy, -10);

    // boss2rots: mother + 4 petal children
    g.vars.pshipflags2 = 0;
    let mother = spawn_obj(&mut g);
    g.objs.aliens[mother as usize].worldx = 0;
    g.objs.aliens[mother as usize].worldy = -60;
    g.objs.aliens[mother as usize].worldz = 1000;
    g.objs.aliens[mother as usize].roty = DEG180;
    let mut kids = [0u16; 4];
    for (i, n) in (2u8..=5).enumerate() {
        let ch = spawn_obj(&mut g);
        assert!(boss_attach_child_to_mother(&mut g, mother, ch, n));
        kids[i] = ch;
    }
    let placed = boss2rots_srou(&mut g, mother, 0);
    assert_eq!(placed, 4);
    assert!(dobossrot_srou(&mut g, mother, 2, 10, 0, 20));
    assert!(dobossrotx4_srou(&mut g, mother, 3, 10, 0, 20));
    assert!(doboss2rot_srou(&mut g, mother, 4, 55, 0, 45, 8));
    // flags 0,1,1 + <<1: rotz=0,roty=180 flips +Z → −Z (mulslog).
    g.objs.aliens[mother as usize].rotz = 0;
    g.objs.aliens[mother as usize].roty = DEG180;
    assert!(dobossrot_srou(&mut g, mother, 2, 0, 0, 20));
    let z = g.objs.aliens[kids[0] as usize].worldz;
    assert!(
        z < 1000,
        "yaw180+roll0 must push +20<<1 behind mother; z={z}"
    );

    // nohitaffect sanity from ship1aexp path already covered elsewhere
    let _ = ASF_NOHITAFFECT;
}
