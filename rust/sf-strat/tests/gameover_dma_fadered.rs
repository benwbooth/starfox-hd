//! Tick 112: FADETONORM + GAMEOVERINIT + DMA_* + FADERED.

use std::cell::Cell;
use std::rc::Rc;

use sf_game::alien::ASF4_INVISIBLE;
use sf_game::dma::{DmaFlush, DmaKind};
use sf_game::vars::GF_PLAYERDYING;
use sf_game::windows::{fade_red_palette, Windows, WINDOW_MODE_WHITE2NORM};
use sf_game::Game;
use sf_path::ids::PATH_ID_GAMEOVER;
use sf_strat::gameover::{fade_to_norm_l, gameover_init_l, SH_GAMESH, SH_OVERSH};

const INITIAL_PLAYER_FORWARD_STEP: i16 = 63;
const LETTER_DEPTH_FROM_PLAYER: i16 = 3400;

struct FadeHooks {
    armed: Rc<Cell<bool>>,
}

impl sf_game::Hooks for FadeHooks {
    fn init_fade_white2norm(&mut self) {
        self.armed.set(true);
    }
}

#[test]
fn fadetonorm_clears_circle_and_arms_hook() {
    let armed = Rc::new(Cell::new(false));
    let mut g = Game::with_hooks(Box::new(FadeHooks {
        armed: Rc::clone(&armed),
    }));
    g.vars.circleanim = 0x1234;
    fade_to_norm_l(&mut g);
    assert_eq!(g.vars.circleanim, 0);
    assert!(armed.get());

    let mut w = Windows::new();
    w.fade_to_norm();
    assert_eq!(w.slots[0].mode, WINDOW_MODE_WHITE2NORM);
    assert_eq!(w.slots[0].wm_val, 31);
}

#[test]
fn gameoverinit_spawns_letters_and_clears_dying() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0, "player() reads slot 0");
    g.objs.aliens[p as usize].active = true;
    g.objs.aliens[p as usize].worldx = 50;
    g.objs.aliens[p as usize].worldy = 10;
    g.objs.aliens[p as usize].worldz = 100;
    g.vars.internal_playpt = p as i16;
    g.vars.gameflags = GF_PLAYERDYING | 0x80;
    g.vars.dotsflag = 1;

    let (game, over) = gameover_init_l(&mut g, p);
    let game = game.expect("games");
    let over = over.expect("over");

    // set_player_cred zeros player pos before letter spawn.
    assert_eq!(g.vars.gameflags & GF_PLAYERDYING, 0);
    assert_eq!(g.vars.dotsflag, -1);
    assert_ne!(g.objs.aliens[p as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[p as usize].worldx, 0);
    assert_eq!(g.objs.aliens[p as usize].worldy, 0);
    assert_eq!(
        g.objs.aliens[p as usize].worldz,
        INITIAL_PLAYER_FORWARD_STEP
    );

    let ga = &g.objs.aliens[game as usize];
    assert_eq!(ga.shape, SH_GAMESH);
    assert_eq!(ga.worldx, -270);
    assert_eq!(ga.worldy, 120);
    assert_eq!(
        ga.worldz,
        INITIAL_PLAYER_FORWARD_STEP + LETTER_DEPTH_FROM_PLAYER
    );
    assert_eq!(ga.hp, 255);
    assert_eq!(ga.sword2, PATH_ID_GAMEOVER as i16);

    let oa = &g.objs.aliens[over as usize];
    assert_eq!(oa.shape, SH_OVERSH);
    assert_eq!(oa.worldx, 270);
    assert_eq!(oa.worldy, 120);
    assert_eq!(
        oa.worldz,
        INITIAL_PLAYER_FORWARD_STEP + LETTER_DEPTH_FROM_PLAYER
    );
}

#[test]
fn dma_sprites_bg2_hpos_flush() {
    let mut d = DmaFlush::new();
    d.dma_sprites();
    d.dma_bg2_voffsets();
    d.dma_hpos();
    assert_eq!(d.sprites, 1);
    assert_eq!(d.bg2_voffsets, 1);
    assert_eq!(d.hpos, 1);
    d.flush(DmaKind::Sprites);
    assert_eq!(d.sprites, 2);
}

#[test]
fn fadered_boosts_red_channel() {
    let mut pal = [0u16; 128];
    pal[0] = (1 << 5) | 2; // G=1, R=2 → |3 → *2 = 6
    pal[1] = 10; // R=10 → *2 = 20
    pal[2] = 20; // R=20 → *2 = 40 → clamp 31
    pal[6 * 16 + 15] = 0x7FFF;
    fade_red_palette(&mut pal);
    assert_eq!(pal[0] & 0x1F, 6);
    assert_eq!(pal[0] & 0x7FE0, 1 << 5);
    assert_eq!(pal[1] & 0x1F, 20);
    assert_eq!(pal[2] & 0x1F, 31);
    assert_eq!(pal[6 * 16 + 15], 0);
}
