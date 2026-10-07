//! Production map dispatch and scheduler contracts backed by the unmodified
//! original-instruction comparisons in sf-oracle/tests/sf1_player_bridge.rs.

use sf_core::{pad, player_view::PlayerViewMode};
use sf_game::{
    alien::{ASF2_COLLDISABLE, ASF4_INVISIBLE},
    vars::*,
    Game, Hooks,
};
use sf_strat::player::{set_player_on_bridge, strat_spawn_player};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Sounds(Rc<RefCell<Vec<u8>>>);
impl Hooks for Sounds {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn schedule_bridge_clear(game: &mut Game) {
    let [low, high, bank, _] = (sf_map::consts::cb::SET_PLAYER_CLEAR_BRIDGE_L - 1).to_le_bytes();
    game.world.map = vec![sf_game::world::op::CODEJSL, low, high, bank];
    game.world.map_loaded = true;
    game.vars.mapptr = 0;
    game.map_exec();
    game.world.map_loaded = false;
}

fn scene() -> (Game, u16) {
    let mut game = Game::new();
    sf_strat::table::register_all(&mut game);
    let player = strat_spawn_player(&mut game).unwrap();
    set_player_on_bridge(&mut game, player);
    game.vars.pshipflags = 0;
    game.vars.pstratflags = 0;
    game.vars.strategy.frame_rate = 4;
    game.vars.strategy.stay_black = -1;
    (game, player)
}

#[test]
fn map_callback_preserves_live_state_until_next_player_visit() {
    let (mut game, player) = scene();
    game.vars.strategy.player_bytes[0] = 77;
    game.vars.game_mode = WATER_MODE;
    game.vars.player_view_mode = PlayerViewMode::Cockpit;
    game.objs.aliens[player as usize].worldz = 1234;
    let before = game.objs.aliens[player as usize];
    let min_y = game.vars.minpmove_y;
    schedule_bridge_clear(&mut game);
    assert_ne!(game.objs.aliens[player as usize].stratptr, before.stratptr);
    assert_eq!(game.objs.aliens[player as usize].worldz, 1234);
    assert_eq!(game.vars.strategy.player_bytes[0], 77);
    assert_eq!(game.vars.pshipflags, 0);
    assert_eq!(game.vars.pstratflags, 0);
    assert_eq!(game.vars.minpmove_y, min_y);
    assert_eq!(game.vars.game_mode, WATER_MODE);
    assert_eq!(game.vars.player_view_mode, PlayerViewMode::Cockpit);
    game.run_strategies();
    assert_eq!(game.vars.strategy.player_bytes[0], 162);
    assert_eq!(game.vars.minpmove_y, -10000);
}

#[test]
fn live_scheduler_runs_duplicate_and_flame_on_birth_then_retires_flame() {
    let (mut game, player) = scene();
    let sounds = Sounds::default();
    game.hooks = Box::new(sounds.clone());
    let duplicate = game.objs.free_head.unwrap();
    let flame = game.objs.aliens[duplicate as usize].next.unwrap();
    schedule_bridge_clear(&mut game);
    for visit in 0_u16..224 {
        game.vars.gameframe = visit.wrapping_sub(1);
        game.vars.pad1 = if (180..200).contains(&visit) {
            pad::Y
        } else {
            0
        };
        sounds.0.borrow_mut().clear();
        game.run_strategies();
        assert_eq!(
            game.vars.strategy.player_bytes[0],
            162_u16.saturating_sub(visit) as u8
        );
        assert_eq!(
            game.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE),
            PSF_NOCTRL | PSF_NOFIRE
        );
        assert_eq!(
            game.vars.pstratflags & (PSTF_INSEQ | PSTF_NOVDISTC),
            PSTF_INSEQ | PSTF_NOVDISTC
        );
        assert_eq!(game.vars.strategy.player_laser_count, 0);
        assert_eq!(
            game.objs.aliens[player as usize].sflags2 & ASF2_COLLDISABLE,
            0
        );
        assert_eq!(
            game.objs.aliens[player as usize].sflags4 & ASF4_INVISIBLE != 0,
            visit >= 163
        );
        let expected = if visit < 163 {
            vec![player]
        } else if (212..221).contains(&visit) {
            vec![player, duplicate, flame]
        } else {
            vec![player, duplicate]
        };
        assert_eq!(game.objs.active_indices(), expected, "list visit {visit}");
        if visit == 163 {
            assert_eq!(game.objs.aliens[duplicate as usize].sbyte1, 49);
        }
        if (212..221).contains(&visit) {
            let child = game.objs.aliens[flame as usize];
            let host = game.objs.aliens[duplicate as usize];
            assert_eq!(child.count, (221 - visit) as u8);
            assert_eq!(
                [child.worldx, child.worldy, child.worldz],
                [host.worldx, host.worldy, host.worldz]
            );
        }
        assert_eq!(
            *sounds.0.borrow(),
            if visit == 212 { vec![50] } else { vec![] },
            "sound visit {visit}"
        );
    }
}
