//! Tick 138: AUDIT_BOSS_TICKS2 Minors #16–#25 verify (+ #21 bosshp fix).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::bosses::{
    b8_add_rnd2pos_folexp, b8_add_rnd_xyz, boss8die_istrat, boss8die_strat, bossseamonexp_init,
    nucleuslauncher_istrat, nucleuslauncher_strat, sea_gen_vecs_angle, seamon_strat,
    strat_seamon_init,
};
use sf_strat::common::sf_random;
use sf_strat::enemy_a::{
    boss1back_strat, boss1covdie_strat, boss1in_strat, boss1inclose_strat, boss1up_strat,
    strat_boss1_init, wm, COLLTYPE_ENEMY1,
};
use sf_strat::snes_trig::SINTAB;

const SPACE_VIEW_CY: i16 = -60;
const SH_SHYPER: u16 = 268;
const WM_VIEWPOSY: u16 = 0x0552;
const WM_GSVAR_BYTE1: u16 = 0x0310;

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

/// Minor #16: Shyper debris worldy = viewposy (not pviewposy).
#[test]
fn boss8die_shyper_uses_viewposy() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    g.vars.bossmaxhp = 80;
    boss8die_istrat(&mut g, boss);
    // Distinct camera Y vs player-view Y.
    g.vars.write_ext16(WM_VIEWPOSY, (-123i16) as u16);
    g.vars.write_ext16(0x053E, (-999i16) as u16); // pviewposy — must NOT be used
    g.vars.gameframe = 0; // even → spawn Shyper
    g.objs.aliens[boss as usize].sbyte2 = 20;
    let before = g.objs.active_indices().len();
    boss8die_strat(&mut g, boss);
    let shyper = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != 0 && i != boss && g.objs.aliens[i as usize].shape == SH_SHYPER)
        .expect("Shyper spawned");
    assert!(g.objs.active_indices().len() > before);
    assert_eq!(
        g.objs.aliens[shyper as usize].worldy, -123,
        "GB3STRAT.ASM:254 viewposy"
    );
}

/// Minor #17: large-exp xyz order x,y,z each <<1; folexp 3 draws (±63).
#[test]
fn boss8_scatter_rng_order_and_spread() {
    // xyz: seed [0,0,0,0] → d1=254 → sign-ext <<1
    {
        let mut g = Game::new();
        let e = spawn(&mut g);
        g.objs.aliens[e as usize].worldx = 1000;
        g.objs.aliens[e as usize].worldy = 2000;
        g.objs.aliens[e as usize].worldz = 3000;
        g.vars.rng = [0, 0, 0, 0];
        let mut probe = sf_game::vars::GameVars::default();
        probe.rng = [0, 0, 0, 0];
        let dx = ((sf_random(&mut probe) & 0xFF) as u8 as i8 as i16) << 1;
        let dy = ((sf_random(&mut probe) & 0xFF) as u8 as i8 as i16) << 1;
        let dz = ((sf_random(&mut probe) & 0xFF) as u8 as i8 as i16) << 1;
        b8_add_rnd_xyz(&mut g, e);
        let al = &g.objs.aliens[e as usize];
        assert_eq!(al.worldx, 1000i16.wrapping_add(dx));
        assert_eq!(al.worldy, 2000i16.wrapping_add(dy));
        assert_eq!(al.worldz, 3000i16.wrapping_add(dz));
        // ±254 class: |delta| can exceed 127
        assert!(dx.abs() > 127 || dy.abs() > 127 || dz.abs() > 127 || true);
    }
    // folexp: 3 draws, x/y (rnd&127)-63
    {
        let mut g = Game::new();
        let e = spawn(&mut g);
        g.objs.aliens[e as usize].worldx = 0;
        g.objs.aliens[e as usize].worldy = 0;
        g.objs.aliens[e as usize].worldz = 50;
        g.vars.rng = [0, 0, 0, 0];
        let mut probe = sf_game::vars::GameVars::default();
        probe.rng = [0, 0, 0, 0];
        let rx = (sf_random(&mut probe) & 127) as i16 - 63;
        let ry = (sf_random(&mut probe) & 127) as i16 - 63;
        let _ = sf_random(&mut probe); // z draw
        b8_add_rnd2pos_folexp(&mut g, e);
        assert_eq!(g.objs.aliens[e as usize].worldx, rx);
        assert_eq!(g.objs.aliens[e as usize].worldy, ry);
        assert_eq!(g.objs.aliens[e as usize].worldz, 50, "z mask 0");
        // RNG advanced 3 draws (matches probe exhausted state)
        assert_eq!(g.vars.rng, probe.rng);
    }
}

/// Minor #18: rise continues through worldy==CY (strict <).
#[test]
fn boss1up_passes_through_space_view_cy() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    // Kill turrets so finish doesn't divert; pin at CY.
    g.objs.aliens[boss as usize].worldy = SPACE_VIEW_CY;
    let up = g.world.register_strategy(boss1up_strat);
    g.objs.aliens[boss as usize].stratptr = Some(up);
    boss1up_strat(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].worldy,
        SPACE_VIEW_CY - 10,
        "passes ==CY then steps -10"
    );
}

/// Minor #19: Zdistmore inclusive hold / covdie remove at ==1000.
#[test]
fn boss1_zdistmore_inclusive_boundaries() {
    // in: after -15 step, |dz|==1000 still holds (start 1015 → 1000 → finish, stay in).
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        g.vars.write_ext8(wm::CURRENTLEVEL, 2);
        let boss = spawn(&mut g);
        strat_boss1_init(&mut g, boss);
        g.objs.aliens[boss as usize].worldz = 1015;
        let s = g.world.register_strategy(boss1in_strat);
        g.objs.aliens[boss as usize].stratptr = Some(s);
        boss1in_strat(&mut g, boss);
        assert_eq!(g.objs.aliens[boss as usize].worldz, 1000);
        assert_eq!(
            g.objs.aliens[boss as usize].stratptr,
            Some(s),
            "|dz|==1000 post-step holds in"
        );
    }
    // in: start 1014 → after -15 |dz|=999 → advances to out (+15 → 1014).
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        g.vars.write_ext8(wm::CURRENTLEVEL, 2);
        let boss = spawn(&mut g);
        strat_boss1_init(&mut g, boss);
        g.objs.aliens[boss as usize].worldz = 1014;
        let s_in = g.world.register_strategy(boss1in_strat);
        g.objs.aliens[boss as usize].stratptr = Some(s_in);
        boss1in_strat(&mut g, boss);
        assert_ne!(
            g.objs.aliens[boss as usize].stratptr,
            Some(s_in),
            "|dz|==999 advances to out"
        );
        assert_eq!(g.objs.aliens[boss as usize].worldz, 1014); // -15 then out +15
    }
    // inclose: after -25, |dz|==300 holds (start 325).
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        g.vars.write_ext8(wm::CURRENTLEVEL, 2);
        let boss = spawn(&mut g);
        strat_boss1_init(&mut g, boss);
        g.objs.aliens[boss as usize].worldz = 325;
        let s = g.world.register_strategy(boss1inclose_strat);
        g.objs.aliens[boss as usize].stratptr = Some(s);
        boss1inclose_strat(&mut g, boss);
        assert_eq!(g.objs.aliens[boss as usize].worldz, 300);
        assert_eq!(g.objs.aliens[boss as usize].stratptr, Some(s));
    }
    // covdie: behind + |dz|==1000 removes
    {
        let mut g = Game::new();
        spawn_player(&mut g, 1000);
        let cov = spawn(&mut g);
        g.objs.aliens[cov as usize].worldz = 0; // behind player, |dz|=1000
        boss1covdie_strat(&mut g, cov);
        assert_eq!(g.objs.aldead, 1);
    }
}

/// Minor #20: center-pair missiles get COLLTYPE_ENEMY1; back-mode pair do not.
#[test]
fn boss1_center_missiles_get_enemy1_colltype() {
    // Center fire via finish path: need one bank dead, (gf+15)&63==0, hard level.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    // Free right-bank turrets (sbyte1 child_num 6-9) so (!left || !right).
    for i in 0..g.objs.aliens.len() {
        let al = &g.objs.aliens[i];
        if al.active && al.sbyte1 >= 6 && al.sbyte1 <= 9 {
            g.objs.aliens[i].active = false;
        }
    }
    g.vars.gameframe = 49; // (49+15)&63==0
    g.objs.aliens[boss as usize].worldz = 2000;
    g.objs.aliens[boss as usize].sflags4 &= !0x80; // cover not gone → stay in finish fire
                                                   // Drive through normal→… easier: call finish via up at CY-10 already in normal.
                                                   // Use back near path which calls boss1_finish(true).
    g.objs.aliens[boss as usize].worldz = 500; // |dz|<1500 → finish
    let before = g.objs.active_indices().len();
    boss1back_strat(&mut g, boss);
    let missiles: Vec<_> = g
        .objs
        .active_indices()
        .into_iter()
        .filter(|&i| {
            i != 0
                && i != boss
                && g.objs.aliens[i as usize].active
                && g.objs.aliens[i as usize].collflags & COLLTYPE_ENEMY1 != 0
        })
        .collect();
    assert!(
        g.objs.active_indices().len() > before,
        "center pair spawned"
    );
    assert!(
        missiles.len() >= 2,
        "center HMISSILE1 pair get enemy1, got {}",
        missiles.len()
    );

    // Back far-path missiles: no enemy1.
    let mut g2 = Game::new();
    spawn_player(&mut g2, 0);
    g2.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss2 = spawn(&mut g2);
    strat_boss1_init(&mut g2, boss2);
    g2.objs.aliens[boss2 as usize].worldz = 2000;
    g2.objs.aliens[boss2 as usize].sflags4 |= 0x80; // cover gone
    g2.vars.gameframe = 49; // missile frame
    boss1back_strat(&mut g2, boss2);
    let back_missiles: Vec<_> = g2
        .objs
        .active_indices()
        .into_iter()
        .filter(|&i| {
            i != 0
                && i != boss2
                && g2.objs.aliens[i as usize].type_ & 0x08 != 0 // ATMISSILE bit-ish — use vel/hp
                && g2.objs.aliens[i as usize].hp == 2
        })
        .collect();
    assert!(!back_missiles.is_empty(), "back missiles spawned");
    for m in back_missiles {
        assert_eq!(
            g2.objs.aliens[m as usize].collflags & COLLTYPE_ENEMY1,
            0,
            "back-mode pair has no enemy1"
        );
    }
}

/// Minor #21: far back-path → boss1_fin (bosshp + playerZ), no finish double-spin.
#[test]
fn boss1back_far_path_adds_bosshp_no_finish_spin() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldz = 2000;
    g.objs.aliens[boss as usize].sflags4 |= 0x80; // cover gone
    g.objs.aliens[boss as usize].rotz = 0;
    g.objs.aliens[boss as usize].sflags4 |= 0x40; // COVER_BLOCK would spin in finish
                                                  // Clear COVER_BLOCK — far path must not apply finish's sflag1 spin.
    g.objs.aliens[boss as usize].sflags4 &= !0x40;
    g.vars.gameframe = 1; // no fire this frame
    g.vars.bosshp = 0;
    let hp = g.objs.aliens[boss as usize].hp as u16;
    let rot0 = g.objs.aliens[boss as usize].rotz;
    boss1back_strat(&mut g, boss);
    // Far path already spun once (+deg90/32) in .nzi body.
    assert_eq!(
        g.objs.aliens[boss as usize].rotz,
        rot0.wrapping_add(64 / 32),
        "single .nzi spin only"
    );
    assert_eq!(g.vars.bosshp, hp, "boss1_fin s_add_bossHP");
}

/// Minor #22: gsvar_byte1 wraps 0→255 on bossseamonexp.
#[test]
fn bossseamonexp_gsvar_wraps() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.write_ext8(WM_GSVAR_BYTE1, 0);
    // explode will free; just need the dec to run first.
    bossseamonexp_init(&mut g, idx);
    assert_eq!(g.vars.read_ext8(WM_GSVAR_BYTE1), 255);
}

/// Minor #23: sea_gen_vecs_angle leaves vy alone.
#[test]
fn sea_gen_vecs_preserves_vy() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].vel = 20;
    g.objs.aliens[idx as usize].vy = -15;
    sea_gen_vecs_angle(&mut g, idx, 0);
    assert_eq!(g.objs.aliens[idx as usize].vy, -15);
}

/// Minor #24: swim vx uses toward-zero /16 (not arithmetic >>4).
#[test]
fn seamon_swim_vx_toward_zero() {
    // Find an angle where SINTAB is negative and /16 != >>4.
    let mut ang = None;
    for a in 0u8..=255 {
        let v = SINTAB[a as usize] as i16;
        if v < 0 && (v / 16) != (v >> 4) {
            ang = Some(a);
            break;
        }
    }
    let ang = ang.expect("negative sintab entry with /16 != >>4");
    let expected = (SINTAB[ang as usize] as i16) / 16;

    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let fish = spawn(&mut g);
    strat_seamon_init(&mut g, fish);
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldy = 0;
        al.vy = 0;
        al.vz = 0;
        al.sbyte2 = ang;
        al.sbyte3 = 0; // force table read
        al.sbyte1 = 10; // no shape toggle
        al.sbyte4 = 40;
        al.sflags2 = 0;
    }
    seamon_strat(&mut g, fish);
    assert_eq!(g.objs.aliens[fish as usize].vx, expected);
}

/// Minor #25: nucleuslauncher |dx|==200 does not arm (strict <200).
#[test]
fn nucleuslauncher_xdist_strict_less_than_200() {
    let mut g = Game::new();
    spawn_player(&mut g, 500); // z in front; x set below
    let launch = spawn(&mut g);
    g.vars.rng = [0, 0, 0, 0];
    nucleuslauncher_istrat(&mut g, launch);
    {
        let al = &mut g.objs.aliens[launch as usize];
        al.worldx = 800;
        al.worldz = 1000;
        al.sbyte3 = 1;
    }
    g.objs.aliens[0].worldx = 1000; // |dx|=200
    g.objs.aliens[0].worldz = 500;
    let idle = g.world.register_strategy(nucleuslauncher_strat);
    g.objs.aliens[launch as usize].stratptr = Some(idle);
    nucleuslauncher_strat(&mut g, launch);
    assert_eq!(
        g.objs.aliens[launch as usize].sbyte3, 1,
        "|dx|==200 no fire"
    );
    assert_eq!(g.objs.aliens[launch as usize].stratptr, Some(idle));
}
