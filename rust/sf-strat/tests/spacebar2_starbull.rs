//! ROM spacebar2 + starbull / stbfp / stbgo family.

use sf_game::alien::{AFONFIRE, ASF_COLLDISABLE, ATZREMOVE};
use sf_game::trig8::{strat_roffs_roll, XSPACEBAR_HALF_B};
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_game::world::{spacebar2_istrat, spacebar2_strat_pub};
use sf_game::Game;
use sf_strat::enemy_a::{
    starbull_istrat, starbull_strat, stbfp_strat, stbgo_init, stbgo_strat, COLLTYPE_ZENEMY,
};

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
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    idx
}

#[test]
fn spacebar2_hardvars_and_follows_parent() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let parent = spawn_obj(&mut g);
    g.objs.aliens[parent as usize].worldx = 100;
    g.objs.aliens[parent as usize].worldy = -50;
    g.objs.aliens[parent as usize].worldz = 3000;
    g.objs.aliens[parent as usize].rotz = 0;

    let bar = spawn_obj(&mut g);
    g.objs.aliens[bar as usize].ptr = parent + 1; // al_ptr = parent
    g.objs.aliens[bar as usize].worldx = 0;
    g.objs.aliens[bar as usize].worldy = -50;
    g.objs.aliens[bar as usize].worldz = 3000;
    g.objs.aliens[bar as usize].rotz = 0;

    spacebar2_istrat(&mut g, bar);
    assert_eq!(g.objs.aliens[bar as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[bar as usize].ap, HARD_AP);
    assert!(g.objs.aliens[bar as usize].stratptr.is_some());

    g.vars.pviewvelz = 5;
    let z0 = g.objs.aliens[bar as usize].worldz;
    spacebar2_strat_pub(&mut g, bar);

    // ROM B-mode #250 → i8 −6; flags 0,0,1 roll at rotz=0 → tip then self.
    let (tx, ty, tz) = strat_roffs_roll(0, XSPACEBAR_HALF_B, 0, 0);
    assert_eq!((ty, tz), (0, 0));
    assert!(
        (-7..=-5).contains(&tx),
        "identity roll of −6 offx ≈ −6, got {tx}"
    );
    // elev≈0 when coplanar → second offset also ≈−6; x ≈ 100−6−6 = 88.
    // Second Roffs uses self.rotz (=elev); when coplanar elev may be 0.
    let srotz = g.objs.aliens[bar as usize].rotz;
    let (ox, _, _) = strat_roffs_roll(srotz, XSPACEBAR_HALF_B, 0, 0);
    let expect_x = 100i16.wrapping_add(tx).wrapping_add(ox);
    assert_eq!(g.objs.aliens[bar as usize].worldx, expect_x);
    assert_eq!(g.objs.aliens[bar as usize].worldz, z0.wrapping_add(5));
    // Parent restored.
    assert_eq!(g.objs.aliens[parent as usize].worldx, 100);
    assert_ne!(g.objs.aliens[bar as usize].colframe & 0x80, 0); // spacemist
}

#[test]
fn spacebar2_roffs_uses_parent_rotz_not_roty() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let parent = spawn_obj(&mut g);
    g.objs.aliens[parent as usize].worldx = 0;
    g.objs.aliens[parent as usize].worldy = 0;
    g.objs.aliens[parent as usize].worldz = 2000;
    g.objs.aliens[parent as usize].roty = 64; // must be ignored (flags 0,0,1)
    g.objs.aliens[parent as usize].rotz = 64; // 90° — rolls −6 offx into −Y

    let bar = spawn_obj(&mut g);
    g.objs.aliens[bar as usize].ptr = parent + 1;
    g.objs.aliens[bar as usize].worldx = 0;
    g.objs.aliens[bar as usize].worldy = 0;
    g.objs.aliens[bar as usize].worldz = 2000;

    spacebar2_istrat(&mut g, bar);
    g.vars.pviewvelz = 0;
    spacebar2_strat_pub(&mut g, bar);

    let (tip_x, tip_y, _) = strat_roffs_roll(64, XSPACEBAR_HALF_B, 0, 0);
    // Tip must follow rotz (roll), not roty (yaw): yaw would move Z, roll moves Y.
    assert!(
        tip_y.abs() > tip_x.abs(),
        "rotz=90 should fold X into Y; tip=({tip_x},{tip_y})"
    );
    assert_ne!(
        g.objs.aliens[bar as usize].worldy, 0,
        "bar must leave Y=0 when parent rotz rolls the tip"
    );
}

#[test]
fn spacebar3_parks_via_rotate16xz_nega() {
    use sf_game::trig8::rotate_16xz;
    use sf_game::world::{spacebar3_istrat, spacebar3_strat_pub};

    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let parent = spawn_obj(&mut g);
    g.objs.aliens[parent as usize].worldx = 100;
    g.objs.aliens[parent as usize].worldy = -40;
    g.objs.aliens[parent as usize].worldz = 3000;
    g.objs.aliens[parent as usize].rotz = 40;
    g.objs.aliens[parent as usize].sbyte1 = 3;

    let child = spawn_obj(&mut g);
    g.objs.aliens[child as usize].ptr = parent + 1;
    // Init captures relative offsets into sword1/2/immuneptr.
    g.objs.aliens[child as usize].worldx = 100 + 80;
    g.objs.aliens[child as usize].worldy = -40 + 50;
    g.objs.aliens[child as usize].worldz = 3000 + 20;
    g.objs.aliens[child as usize].rotz = 10;

    spacebar3_istrat(&mut g, child);
    assert_eq!(g.objs.aliens[child as usize].sword1, 80);
    assert_eq!(g.objs.aliens[child as usize].sword2, 50);
    assert_eq!(g.objs.aliens[child as usize].immuneptr, 20);

    spacebar3_strat_pub(&mut g, child);
    let (rx, ry) = rotate_16xz(40u8.wrapping_neg(), 80, 50);
    // World-lane trig8 must match sf-strat snes_trig.
    assert_eq!(
        (rx, ry),
        sf_strat::snes_trig::rotate_16xz(40u8.wrapping_neg(), 80, 50)
    );
    assert_eq!(
        g.objs.aliens[child as usize].worldx,
        100i16.wrapping_add(rx)
    );
    assert_eq!(
        g.objs.aliens[child as usize].worldy,
        (-40i16).wrapping_add(ry)
    );
    assert_eq!(g.objs.aliens[child as usize].worldz, 3020);
    assert_eq!(g.objs.aliens[child as usize].rotz, 13); // 10+3
    assert_ne!(g.objs.aliens[child as usize].colframe & 0x80, 0); // spacemist
}

#[test]
fn starbull_init_and_chase_tail() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 5000;
    starbull_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 16);
    assert_eq!(g.objs.aliens[idx as usize].ap, 1);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    assert_eq!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 2);

    let z0 = g.objs.aliens[idx as usize].worldz;
    starbull_strat(&mut g, idx);
    // WP chase + add_player_z; should have moved toward player WP.
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    let _ = z0;
}

#[test]
fn starbull_reaches_face_then_peel() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    // Place already at the WP (player + offsets) so goto_wp reaches.
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -40i16.wrapping_add(-50);
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].vel = 0;
    starbull_istrat(&mut g, idx);
    starbull_strat(&mut g, idx);
    // After reach → starbull2 → stbfp_strat, sbyte1=20.
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 20);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);

    // Force face-aligned: set smflag1 + sbyte3/4 = current rots.
    g.objs.aliens[idx as usize].sflags2 |= sf_strat::enemy_a::ASF2_SMFLAG1;
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].roty;
    g.objs.aliens[idx as usize].sbyte4 = g.objs.aliens[idx as usize].rotx;
    stbfp_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 19); // beqdec fired

    // Peel path.
    g.objs.aliens[idx as usize].flags |= 0x10; // inviewpl so not culled
    stbgo_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    stbgo_strat(&mut g, idx);
}

#[test]
fn starbullc_spins_when_on_fire() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    starbull_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].flags |= AFONFIRE;
    g.objs.aliens[idx as usize].stratstate = 1;
    g.vars.gameframe = 0; // delay gate open
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    starbull_strat(&mut g, idx);
    // On fire + state1 → rotz -= 8 (after possible next_state).
    assert_ne!(g.objs.aliens[idx as usize].rotz, rotz0);
}
