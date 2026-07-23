//! Tick 139: AUDIT_BOSS_TICKS2 Minors #26/#28 + sea_make_splash gap.

use sf_game::alien::{ASF3_REALOBJ, ASF_COLLDISABLE};
use sf_game::Game;
use sf_strat::bosses::{
    boss8a_init, boss8b_init, nucleuslauncher_istrat, sea_make_splash, sea_make_splash_surface,
    seamon_strat, strat_boss8_init, strat_bossseamon_init, strat_seamon_init,
};
use sf_strat::enemy_a::wm;

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
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// Known gap: sea_make_splash → makesplash_srou (no longer a no-op).
#[test]
fn sea_make_splash_spawns_child() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    g.objs.aliens[parent as usize].worldz = 1000;
    g.objs.aliens[parent as usize].worldy = -20;
    let before = g.objs.active_indices().len();
    sea_make_splash(&mut g, parent);
    assert_eq!(g.objs.active_indices().len(), before + 1);
    let splash = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != parent)
        .expect("splash");
    assert_ne!(g.objs.aliens[splash as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[splash as usize].worldz, 995);
}

/// Seamon landing forces splash worldy=0 (GASTRATS.ASM:2101).
#[test]
fn seamon_landing_splash_snaps_worldy_zero() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let fish = spawn(&mut g);
    strat_seamon_init(&mut g, fish);
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldy = -10;
        al.vy = 0;
        al.vx = 0;
        al.vz = 0;
        al.sflags2 = 0; // not yet latched
        al.sbyte3 = 40;
        al.sbyte4 = 40;
    }
    let before = g.objs.active_indices().len();
    seamon_strat(&mut g, fish);
    assert!(
        g.objs.active_indices().len() > before,
        "landing spawns splash"
    );
    let splash = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != 0 && i != fish)
        .expect("splash child");
    assert_eq!(g.objs.aliens[splash as usize].worldy, 0);
    assert_ne!(g.objs.aliens[fish as usize].sflags2 & 0x20, 0); // SEA_SFLAG1
}

#[test]
fn sea_make_splash_surface_forces_y_zero() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    g.objs.aliens[parent as usize].worldy = -50;
    sea_make_splash_surface(&mut g, parent);
    let splash = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != parent)
        .expect("splash");
    assert_eq!(g.objs.aliens[splash as usize].worldy, 0);
}

/// Minor #26: boss8 open clears colldisable+sets hitflash; close sets both off.
/// Port folds ROM s_docoll into coldet, so ASF_COLLDISABLE ≡ collstrat=0.
#[test]
fn boss8_colldisable_tracks_open_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss8_init(&mut g, boss);
    // Init/wait: invulnerable (ROM collstrat=0 from s_set_alptrs).
    assert_ne!(
        g.objs.aliens[boss as usize].sflags & ASF_COLLDISABLE,
        0,
        "wait: colldisable"
    );
    assert!(g.objs.aliens[boss as usize].collstratptr.is_none());

    // Latch beam sflag1 so boss8a_strat doesn't immediately re-close.
    for i in 0..g.objs.aliens.len() {
        let al = &g.objs.aliens[i];
        if al.active && al.sbyte1 >= 2 && al.sbyte1 <= 4 {
            g.objs.aliens[i].sflags2 |= 0x10; // B8_SFLAG1
        }
    }

    boss8a_init(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].sflags & ASF_COLLDISABLE,
        0,
        "open: damageable"
    );
    assert!(
        g.objs.aliens[boss as usize].collstratptr.is_some(),
        "open: hitflash collstrat"
    );

    boss8b_init(&mut g, boss);
    assert_ne!(
        g.objs.aliens[boss as usize].sflags & ASF_COLLDISABLE,
        0,
        "close: colldisable"
    );
    assert!(g.objs.aliens[boss as usize].collstratptr.is_none());

    // Coldet skips colldisable objects (ASF_COLLDISABLE gate).
    g.coldet_generate_list();
    assert!(
        !g.coldet.list.iter().any(|e| e.alien == boss),
        "closed boss absent from collist"
    );
}

/// Minor #28: Istrat fall-through runs body same tick (bossseamon + launcher).
#[test]
fn deferred_istrat_fallthrough_same_tick() {
    // bossseamon: init calls body → leaves state 0 with sbyte3 decremented.
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        let boss = spawn(&mut g);
        g.vars.rng = [0, 0, 0, 0];
        strat_bossseamon_init(&mut g, boss);
        assert_eq!(g.objs.aliens[boss as usize].stratstate, 0);
        // Init sets sbyte3=60 then body decs once → 59 (unless delay→state1).
        assert!(
            g.objs.aliens[boss as usize].sbyte3 < 60
                || g.objs.aliens[boss as usize].stratstate != 0,
            "body ran on spawn tick"
        );
        assert!(g.objs.aliens[boss as usize].stratptr.is_some());
    }
    // nucleuslauncher: istrat runs wallrot placement (worldx/z leave defaults).
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        let launch = spawn(&mut g);
        g.vars.rng = [0, 0, 0, 0];
        let zx0 = g.objs.aliens[launch as usize].worldz;
        nucleuslauncher_istrat(&mut g, launch);
        // wallrot from sword2=BOSS8_CIRC places z away from 0 default.
        assert_ne!(
            g.objs.aliens[launch as usize].worldz, zx0,
            "first wallrot same tick"
        );
        assert!(g.objs.aliens[launch as usize].stratptr.is_some());
    }
}
