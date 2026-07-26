//! Damaged-carrier countdown circle (GCSTRATS.ASM `ship0cdown_strat`).

use sf_core::screen_fill_circle::{
    ScreenFillCircleCenter, ScreenFillCirclePhase, ScreenFillCircleScope, BOSS_RADIUS_SPEED,
};
use sf_game::Game;
use sf_strat::enemy_a::{ship0cdown_istrat, ship0cdown_strat, ASF2_SFLAG1};

const CARRIER_POSITION: [i16; 3] = [-300, 200, 1500];

#[test]
fn carrier_countdown_starts_the_last_stage_circle_on_its_own_object() {
    let mut game = Game::new();
    let carrier = game.objs.alloc().expect("carrier");
    {
        let object = &mut game.objs.aliens[carrier as usize];
        object.worldx = CARRIER_POSITION[0];
        object.worldy = CARRIER_POSITION[1];
        object.worldz = CARRIER_POSITION[2];
    }
    ship0cdown_istrat(&mut game, carrier);
    game.objs.aliens[carrier as usize].sbyte1 = 1;

    ship0cdown_strat(&mut game, carrier);

    assert_ne!(game.objs.aliens[carrier as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(
        game.vars.screen_fill_circle.center,
        ScreenFillCircleCenter::Object(carrier + 1)
    );
    assert_eq!(
        game.vars.screen_fill_circle.phase,
        ScreenFillCirclePhase::LastStageExpanding
    );
    assert_eq!(
        game.vars.screen_fill_circle.scope,
        ScreenFillCircleScope::Background
    );
    assert_eq!(
        game.vars.screen_fill_circle.radius,
        BOSS_RADIUS_SPEED as u16
    );
    assert_eq!(game.vars.strategy.circle_object, (carrier + 1) as i16);
}
