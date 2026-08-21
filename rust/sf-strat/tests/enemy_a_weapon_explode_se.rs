//! Tick 150: AUDIT_ENEMY_A High #1/#2 (relslowlaser + notdelay bit-count) +
//! AUDIT_HUD Critical #8/#9 (damage/destruct range SE + noexpsnd gate).

use sf_game::alien::{ACF_COLLTYPE1, ACF_COLLTYPE4, ASF3_REALOBJ, ASF_COLLIDE};
use sf_game::game::{Game, Hooks};
use sf_strat::enemy_a::{
    frame_tick_mod, strat_explode, strat_fire_relslowlaser, strat_hit_flash,
    strat_relslowelaser_speed, wm, ASF2_NOEXPSND,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<u8>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
    fn trig_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

fn spawn_player(g: &mut Game, x: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = -40;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// High #1: `fire_relslowElaser` — doelaserspeed 48@L1 / 60 else, life 40, AP 2,
/// colltypes enemyweap|laser (GSTRATS.ASM:2548-2561).
#[test]
fn relslowlaser_speed_life_colltypes_match_rom() {
    for (lvl, expect_speed) in [(1u8, 48u8), (2u8, 60u8), (3u8, 60u8)] {
        let mut g = Game::new();
        g.vars.write_ext8(wm::CURRENTLEVEL, lvl);
        assert_eq!(strat_relslowelaser_speed(&g), expect_speed);

        spawn_player(&mut g, 0, 0);
        let firer = spawn(&mut g);
        g.objs.aliens[firer as usize].worldx = 0;
        g.objs.aliens[firer as usize].worldz = 500;

        let before = g.objs.aliens.iter().filter(|a| a.active).count();
        strat_fire_relslowlaser(&mut g, firer, 0, 0);
        let after = g.objs.aliens.iter().filter(|a| a.active).count();
        assert_eq!(after, before + 2, "level {lvl}: bolt + muzzle flash");

        let shot = g
            .objs
            .aliens
            .iter()
            .enumerate()
            .find(|(i, a)| a.active && *i as u16 != 0 && *i as u16 != firer)
            .expect("projectile");
        assert_eq!(shot.1.vel, expect_speed, "level {lvl} speed");
        assert_eq!(shot.1.count, 40, "level {lvl} lifecnt");
        assert_eq!(shot.1.ap, 2, "level {lvl} AP");
        assert_ne!(shot.1.collflags & ACF_COLLTYPE4, 0, "level {lvl} enemyweap");
        assert_ne!(
            shot.1.collflags & ACF_COLLTYPE1,
            0,
            "level {lvl} laser colltype"
        );
    }
}

/// High #2: `s_jmp_notdelay N` = bit-count period 2^N, not modulus N.
#[test]
fn frame_tick_mod_is_bit_count_not_modulus() {
    let mut g = Game::new();
    // N=1 → period 2: true on even frames.
    g.vars.gameframe = 0;
    assert!(frame_tick_mod(&g, 1));
    g.vars.gameframe = 1;
    assert!(!frame_tick_mod(&g, 1));
    g.vars.gameframe = 2;
    assert!(frame_tick_mod(&g, 1));

    // N=2 → period 4 (modulus-2 would fire every other frame — wrong).
    g.vars.gameframe = 0;
    assert!(frame_tick_mod(&g, 2));
    g.vars.gameframe = 2;
    assert!(!frame_tick_mod(&g, 2));
    g.vars.gameframe = 4;
    assert!(frame_tick_mod(&g, 2));

    // N=3 → period 8.
    g.vars.gameframe = 0;
    assert!(frame_tick_mod(&g, 3));
    g.vars.gameframe = 3;
    assert!(!frame_tick_mod(&g, 3));
    g.vars.gameframe = 8;
    assert!(frame_tick_mod(&g, 3));
}

/// Critical #8: non-fatal hitflash → $24/$25/$26 by xz range (<1000/<2000/else).
#[test]
fn hit_flash_plays_damage_se_by_range() {
    for (dz, expect) in [(500i16, 0x24u8), (1500, 0x25), (2500, 0x26)] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
        spawn_player(&mut g, 0, 0);
        let idx = spawn(&mut g);
        let attacker = spawn(&mut g);
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].worldz = dz;
        g.objs.aliens[idx as usize].hp = 5;
        g.objs.aliens[idx as usize].collobjptr = attacker;
        g.objs.aliens[idx as usize].collcount = 1;
        g.objs.aliens[attacker as usize].ap = 1;
        g.objs.aliens[idx as usize].sflags |= ASF_COLLIDE;

        strat_hit_flash(&mut g, idx);

        assert_eq!(
            *log.borrow(),
            vec![expect],
            "dz={dz} expected ${expect:02x}"
        );
        assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    }
}

/// Critical #9: explode → $21/$22/$23 by range; ASF2_NOEXPSND silences.
#[test]
fn explode_plays_destruct_se_by_range_unless_noexpsnd() {
    for (dz, expect) in [(500i16, 0x21u8), (1500, 0x22), (2500, 0x23)] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
        spawn_player(&mut g, 0, 0);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldx = 0;
        g.objs.aliens[idx as usize].worldz = dz;
        g.objs.aliens[idx as usize].flags |= 0x10; // AF_INVIEW_PL

        strat_explode(&mut g, idx);

        assert_eq!(
            *log.borrow(),
            vec![expect],
            "dz={dz} expected ${expect:02x}"
        );
    }

    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    g.objs.aliens[idx as usize].flags |= 0x10; // AF_INVIEW_PL
    strat_explode(&mut g, idx);
    assert!(
        log.borrow().is_empty(),
        "NOEXPSND must silence destruct SE, got {:?}",
        *log.borrow()
    );
}
