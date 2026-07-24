//! Tick 130: item6 shieldup + wall MoveWall SE + player_start_init clear.

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_strat::common::{sv, StratRam};
use sf_strat::enemies_ground::{item6_istrat, wall1_strat, walll_istrat, wallnothit};
use sf_strat::player::{player_start_init, strat_player};
use std::cell::RefCell;
use std::rc::Rc;

const PSF2_WIRESHIP: u8 = 2;
const HIT_PRIMARY: u8 = 1;
const WALL_LEFT_SHAPE: u16 = 457;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SndEvent {
    PlaySe(u8),
    MakeSnd(PosSndFamilyId, i16, i16),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<SndEvent>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(SndEvent::PlaySe(id));
    }
    fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
        self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
    }
}

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    {
        let al = &mut g.objs.aliens[0];
        al.active = true;
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 0;
        al.sflags3 |= sf_game::alien::ASF3_REALOBJ;
    }
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = 0;
    g.vars.internal_playpt = 0;
}

#[test]
fn item6_sets_shieldup_and_clears_pnumhits() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);
    player_start_init(&mut g);
    let normal_shape = g.vars.sv_u16(sv::PLAYERSHAPE);
    g.vars.set_sv_u8(sv::PNUMHITS, 5);

    let w = g.objs.alloc().expect("item");
    {
        let al = &mut g.objs.aliens[w as usize];
        al.active = true;
        al.worldx = 0;
        al.worldy = -40;
        al.worldz = 90; // +20 drift -> 110 < 120
    }
    item6_istrat(&mut g, w);
    assert_ne!(g.vars.pshipflags2 & PSF2_WIRESHIP, 0);
    assert_eq!(g.vars.shieldup, 1);
    assert_eq!(g.vars.sv_u8(sv::PNUMHITS), 0);
    assert_ne!(g.vars.sv_u16(sv::PLAYERSHAPE), normal_shape);
    assert!(log.borrow().contains(&SndEvent::PlaySe(0x16)));
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn wire_ship_expiration_selects_normal_mesh_before_clearing_the_powerup() {
    let mut g = Game::new();
    spawn_player(&mut g);
    g.objs.aliens[0].hp = 100;
    player_start_init(&mut g);
    let normal_shape = g.vars.sv_u16(sv::PLAYERSHAPE);

    let item = g.objs.alloc().expect("item");
    g.objs.aliens[item as usize].worldy = -40;
    g.objs.aliens[item as usize].worldz = 90;
    item6_istrat(&mut g, item);
    assert_ne!(g.vars.sv_u16(sv::PLAYERSHAPE), normal_shape);

    g.vars.set_sv_u8(sv::PNUMHITS, 3);
    g.vars.wireendflash = 1;
    strat_player(&mut g, 0);

    assert_eq!(g.vars.wireendflash, 0);
    assert_eq!(g.vars.shieldup, 0);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), normal_shape);
    assert_ne!(g.vars.pshipflags2 & PSF2_WIRESHIP, 0);

    strat_player(&mut g, 0);
    assert_eq!(g.vars.pshipflags2 & PSF2_WIRESHIP, 0);
}

#[test]
fn player_start_init_clears_shieldup() {
    let mut g = Game::new();
    g.vars.shieldup = 1;
    g.vars.wireendflash = 40;
    g.vars.pshipflags2 = PSF2_WIRESHIP;
    player_start_init(&mut g);
    assert_eq!(g.vars.shieldup, 0);
    assert_eq!(g.vars.wireendflash, 0);
    assert_eq!(g.vars.pshipflags2, 0);
}

#[test]
fn wall_latch_fires_movewall_make_snd() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);

    let w = g.objs.alloc().expect("wall");
    {
        let al = &mut g.objs.aliens[w as usize];
        al.active = true;
        al.worldx = 50;
        al.worldy = 0;
        al.worldz = 100;
        al.animframe = 0x81; // lean left -> wallleft_i
        al.roty = 128; // deg180
    }
    wallnothit(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].shape, WALL_LEFT_SHAPE);
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::MoveWall, 50, 100))),
        "wallleft_i must jsl movewallsound_l; got {:?}",
        log.borrow()
    );
}

#[test]
fn wall_hit_flip_plays_the_source_switch_cue() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g);

    let wall = g.objs.alloc().expect("wall");
    g.objs.aliens[wall as usize].worldz = 2_000;
    walll_istrat(&mut g, wall);
    log.borrow_mut().clear();
    g.objs.aliens[wall as usize].hitflags = HIT_PRIMARY;
    wall1_strat(&mut g, wall);

    assert!(log.borrow().contains(&SndEvent::PlaySe(0x57)));
}
