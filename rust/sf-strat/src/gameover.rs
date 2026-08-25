//! Game-over screen init (ROM GSTRATS.ASM `gameoverinit_l`).

use sf_game::alien::ASF4_INVISIBLE;
use sf_game::vars::GF_PLAYERDYING;
use sf_game::Game;
use sf_path::ids::PATH_ID_GAMEOVER;

use crate::common::{initgame_strats_l, strat_make_obj};
use crate::player::{player_start_init, set_player_cred};

/// Shape ids for the "GAME" / "OVER" letterforms (shape_data).
pub const SH_GAMESH: u16 = 245;
pub const SH_OVERSH: u16 = 246;

/// Absolute world X for the two letter objects (ROM `s_set_alvar #±270`).
const GAME_WORLD_X: i16 = -270;
const OVER_WORLD_X: i16 = 270;
/// Added to player Y/Z after `s_copy_pos`.
const LETTER_OY: i16 = 120;
const LETTER_OZ: i16 = 3400;

fn spawn_gameover_letter(g: &mut Game, shape: u16, world_x: i16) -> Option<u16> {
    let idx = strat_make_obj(g, shape)?;
    let (by, bz) = g
        .objs
        .player()
        .map(|p| (p.worldy, p.worldz))
        .unwrap_or((0, 0));
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = 255;
        al.ap = 0;
        // ROM: copy_pos then overwrite worldx with absolute ±270.
        al.worldx = world_x;
        al.worldy = by.wrapping_add(LETTER_OY);
        al.worldz = bz.wrapping_add(LETTER_OZ);
        al.sword2 = PATH_ID_GAMEOVER as i16;
    }
    let start = g.hooks.resolve_path_start(PATH_ID_GAMEOVER);
    if start != 0 {
        g.objs.aliens[idx as usize].sword2 = start as i16;
    }
    Some(idx)
}

/// ROM `gameoverinit_l` (GSTRATS.ASM:3255):
/// - point map at waitmap (HD: leave mapptr; shell owns map load)
/// - `initgame_l` → HD [`initgame_strats_l`] + [`player_start_init`] + [`set_player_cred`]
/// - clear `gf_playerdying`, disable particles (HD: no particle flag)
/// - spawn gamesh/oversh path objects
/// - black out pal0 rows 0..6 then load gameoverpal (HD: dotsflag = -1)
pub fn gameover_init_l(g: &mut Game, player_idx: u16) -> (Option<u16>, Option<u16>) {
    initgame_strats_l(g);
    player_start_init(g);
    if (player_idx as usize) < g.objs.aliens.len() {
        g.objs.aliens[player_idx as usize].active = true;
        g.vars.internal_playpt = player_idx as i16;
        set_player_cred(g, player_idx);
    }
    g.vars.gameflags &= !GF_PLAYERDYING;
    g.vars.dotsflag = -1;

    let game = spawn_gameover_letter(g, SH_GAMESH, GAME_WORLD_X);
    let over = spawn_gameover_letter(g, SH_OVERSH, OVER_WORLD_X);
    if (player_idx as usize) < g.objs.aliens.len() {
        g.objs.aliens[player_idx as usize].sflags4 |= ASF4_INVISIBLE;
    }
    (game, over)
}

/// ROM `fadetonorm_l` (GSTRATS.ASM:1021) — clear circle wipe, arm white→norm.
pub fn fade_to_norm_l(g: &mut Game) {
    g.vars.circleanim = 0;
    g.hooks.init_fade_white2norm();
}
