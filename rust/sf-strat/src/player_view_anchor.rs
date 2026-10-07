//! Player-camera anchors from PSTRATS.ASM, before the shared depth chase.
//!
//! Keep the separate arithmetic shifts: multiplying by a fraction rounds
//! negative coordinates differently from the original `perc*A_l` routines.

use crate::common::{strat_perc62, strat_perc75, strat_perc87};
use sf_core::player_view::PlayerViewMode;
use sf_game::Game;

pub(crate) const SPACE_VIEW_CENTER_Y: i16 = -60;

/// The strategy owning the anchor, independent of the selectable camera mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightViewAnchor {
    Space,
    Surface,
    Underground,
}

/// Update only the two lateral anchor coordinates. The caller still owns the
/// subsequent `viewmove_srou` engine sound, depth chase and cockpit override.
pub fn apply_flight_view_anchor(game: &mut Game, player: u16, flight: FlightViewAnchor) {
    let actor = &game.objs.aliens[player as usize];
    let center = game.vars.strategy.view_center_y;
    let [x, y] = match flight {
        FlightViewAnchor::Space if game.vars.player_view_mode == PlayerViewMode::Cockpit => {
            [actor.worldx, actor.worldy]
        }
        FlightViewAnchor::Space => [
            strat_perc75(actor.worldx),
            strat_perc62(actor.worldy.wrapping_sub(SPACE_VIEW_CENTER_Y))
                .wrapping_add(SPACE_VIEW_CENTER_Y),
        ],
        FlightViewAnchor::Surface => [
            strat_perc87(actor.worldx),
            strat_perc75(actor.worldy.wrapping_sub(center)).wrapping_add(center),
        ],
        FlightViewAnchor::Underground => [strat_perc87(actor.worldx), center],
    };
    game.vars.strategy.player_view_position[0] = x;
    game.vars.strategy.player_view_position[1] = y;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_completed_space_cockpit_copies_the_ship_position() {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        game.objs.aliens[player as usize].worldx = -1;
        game.objs.aliens[player as usize].worldy = -61;
        game.vars.strategy.view_center_y = 123;
        game.vars.strategy.player_view_position[2] = 456;
        for mode in [
            PlayerViewMode::Exterior,
            PlayerViewMode::CloseExterior,
            PlayerViewMode::EnteringCockpit,
            PlayerViewMode::Cockpit,
            PlayerViewMode::LeavingCockpit,
        ] {
            game.vars.player_view_mode = mode;
            apply_flight_view_anchor(&mut game, player, FlightViewAnchor::Space);
            assert_eq!(
                game.vars.strategy.player_view_position,
                if mode == PlayerViewMode::Cockpit {
                    [-1, -61, 456]
                } else {
                    [-2, -62, 456]
                }
            );
            assert_eq!(game.vars.strategy.view_center_y, 123);
        }
    }

    #[test]
    fn surface_tracks_live_center_while_underground_keeps_it_fixed() {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        game.objs.aliens[player as usize].worldx = -1;
        game.objs.aliens[player as usize].worldy = -61;
        game.vars.strategy.view_center_y = -50;
        game.vars.strategy.player_view_position[2] = 456;
        // The shared depth routine, not the surface anchor, owns the later
        // cockpit override. This remains true with depth movement disabled.
        game.vars.player_view_mode = PlayerViewMode::Cockpit;
        apply_flight_view_anchor(&mut game, player, FlightViewAnchor::Surface);
        assert_eq!(game.vars.strategy.player_view_position, [-3, -59, 456]);
        apply_flight_view_anchor(&mut game, player, FlightViewAnchor::Underground);
        assert_eq!(game.vars.strategy.player_view_position, [-3, -50, 456]);
    }

    #[test]
    fn center_subtraction_and_addition_wrap_before_and_after_scaling() {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        game.objs.aliens[player as usize].worldy = i16::MIN;
        game.vars.strategy.view_center_y = i16::MAX;
        apply_flight_view_anchor(&mut game, player, FlightViewAnchor::Surface);
        assert_eq!(game.vars.strategy.player_view_position[1], i16::MAX);
        game.objs.aliens[player as usize].worldy = i16::MAX;
        apply_flight_view_anchor(&mut game, player, FlightViewAnchor::Space);
        assert_eq!(game.vars.strategy.player_view_position[1], -20504);
    }
}
