//! Source-authored smoke cadence, placement, and velocity regressions.

use sf_game::alien::{AFONFIRE, ASF3_REALOBJ};
use sf_game::{Game, Hooks};
use sf_strat::enemies_ground::{
    bazfall_istrat, bazfall_strat, cruiser1fall_strat, cruiser2fire_strat, fly4_istrat, fly4_strat,
    move_strat, volplasma_strat, volrock_strat, woodsgo_strat,
};
use sf_strat::enemy_a::{starbull_istrat, starbull_strat};
use sf_strat::snes_trig::strat_roffs_full_i16;
use std::cell::RefCell;
use std::rc::Rc;

// Generated USHAPES.ASM smoke, distinct from the fire/burn-mark shape.
const SMOKE_SHAPE: u16 = 358;
const STARBULL_DAMAGE_SMOKE_HP: u8 = 12;
const WALKER_DAMAGE_SMOKE_HP: u8 = 4;
const FLY_DAMAGE_SMOKE_HP: u8 = 1;
const WALKER_DAMAGE_SMOKE_LOCAL_Y: i16 = -100;
const EVERY_OTHER_FRAME_PERIOD: u16 = 2;
const EVERY_FOURTH_FRAME_PERIOD: u16 = 4;
const EVERY_EIGHTH_FRAME_PERIOD: u16 = 8;
const CRUISER_NEAR_DESTRUCTION_SOUND: u8 = 33;
const CRUISER_MID_DESTRUCTION_SOUND: u8 = 34;

fn setup() -> (Game, u16) {
    let mut game = Game::new();
    let player = game.objs.alloc().expect("player");
    assert_eq!(player, 0);
    game.objs.aliens[player as usize].sflags3 |= ASF3_REALOBJ;
    game.vars.internal_playpt = 0;

    let object = game.objs.alloc().expect("test object");
    {
        let object_state = &mut game.objs.aliens[object as usize];
        object_state.worldx = 100;
        object_state.worldy = -200;
        object_state.worldz = 3000;
    }
    (game, object)
}

fn smoke_count(game: &Game) -> usize {
    game.objs
        .aliens
        .iter()
        .filter(|object| object.active && object.shape == SMOKE_SHAPE)
        .count()
}

fn latest_smoke(game: &Game) -> u16 {
    game.objs
        .active_indices()
        .into_iter()
        .find(|&object| game.objs.aliens[object as usize].shape == SMOKE_SHAPE)
        .expect("source smoke")
}

fn assert_cadence(game: &mut Game, object: u16, tick: fn(&mut Game, u16), period: u16) {
    game.vars.gameframe = 0;
    tick(game, object);
    assert_eq!(smoke_count(game), 1, "frame zero emits");

    game.vars.gameframe = 1;
    tick(game, object);
    assert_eq!(smoke_count(game), 1, "off-cadence frame stays dry");

    game.vars.gameframe = period;
    tick(game, object);
    assert_eq!(smoke_count(game), 2, "next cadence boundary emits");
}

#[test]
fn periodic_smoke_emitters_use_every_other_and_every_fourth_frames() {
    let (mut game, object) = setup();
    bazfall_istrat(&mut game, object);
    assert_cadence(&mut game, object, bazfall_strat, EVERY_OTHER_FRAME_PERIOD);

    let (mut game, object) = setup();
    assert_cadence(
        &mut game,
        object,
        volplasma_strat,
        EVERY_FOURTH_FRAME_PERIOD,
    );

    let (mut game, object) = setup();
    assert_cadence(&mut game, object, volrock_strat, EVERY_FOURTH_FRAME_PERIOD);

    let (mut game, object) = setup();
    assert_cadence(
        &mut game,
        object,
        cruiser1fall_strat,
        EVERY_FOURTH_FRAME_PERIOD,
    );

    let (mut game, object) = setup();
    assert_cadence(
        &mut game,
        object,
        cruiser2fire_strat,
        EVERY_OTHER_FRAME_PERIOD,
    );
}

#[test]
fn woods_missile_smoke_reverses_and_amplifies_the_source_velocity() {
    let (mut game, object) = setup();
    {
        let missile = &mut game.objs.aliens[object as usize];
        missile.vx = -3;
        missile.vy = 5;
        missile.vz = -7;
        missile.sbyte1 = 10;
    }

    game.vars.gameframe = 0;
    let source_velocity = {
        let missile = game.objs.aliens[object as usize];
        (missile.vx, missile.vy, missile.vz)
    };
    woodsgo_strat(&mut game, object);
    let smoke = &game.objs.aliens[latest_smoke(&game) as usize];
    assert_eq!(smoke.vx, source_velocity.0.wrapping_neg());
    assert_eq!(smoke.vy, source_velocity.1.wrapping_neg());
    assert_eq!(smoke.vz, source_velocity.2.wrapping_neg().wrapping_mul(4));

    game.vars.gameframe = 1;
    woodsgo_strat(&mut game, object);
    assert_eq!(smoke_count(&game), 1, "off-cadence frame stays dry");

    game.vars.gameframe = EVERY_FOURTH_FRAME_PERIOD;
    woodsgo_strat(&mut game, object);
    assert_eq!(smoke_count(&game), 2, "next cadence boundary emits");
}

#[test]
fn walker_damage_smoke_uses_threshold_cadence_and_rotated_local_offset() {
    let (mut game, object) = setup();
    {
        let walker = &mut game.objs.aliens[object as usize];
        walker.hp = WALKER_DAMAGE_SMOKE_HP;
        walker.vx = 3;
        walker.vy = -2;
        walker.vz = 5;
        walker.rotz = 16;
        walker.rotx = 32;
        walker.roty = 64;
    }
    assert_cadence(&mut game, object, move_strat, EVERY_EIGHTH_FRAME_PERIOD);
    assert_ne!(game.objs.aliens[object as usize].flags & AFONFIRE, 0);

    let walker = game.objs.aliens[object as usize];
    let smoke = game.objs.aliens[latest_smoke(&game) as usize];
    let (offset_x, offset_y, offset_z) = strat_roffs_full_i16(
        walker.rotz,
        walker.rotx,
        walker.roty,
        0,
        WALKER_DAMAGE_SMOKE_LOCAL_Y,
        0,
    );
    assert_eq!(smoke.worldx, walker.worldx.wrapping_add(offset_x));
    assert_eq!(smoke.worldy, walker.worldy.wrapping_add(offset_y));
    assert_eq!(smoke.worldz, walker.worldz.wrapping_add(offset_z));

    let (mut healthy_game, healthy) = setup();
    healthy_game.objs.aliens[healthy as usize].hp = WALKER_DAMAGE_SMOKE_HP + 1;
    healthy_game.vars.gameframe = 0;
    move_strat(&mut healthy_game, healthy);
    assert_eq!(smoke_count(&healthy_game), 0);
    assert_eq!(
        healthy_game.objs.aliens[healthy as usize].flags & AFONFIRE,
        0,
    );
}

#[test]
fn fly_and_starbull_damage_smoke_use_authored_thresholds() {
    let (mut game, object) = setup();
    fly4_istrat(&mut game, object);
    game.objs.aliens[object as usize].hp = FLY_DAMAGE_SMOKE_HP;
    assert_cadence(&mut game, object, fly4_strat, EVERY_FOURTH_FRAME_PERIOD);
    assert_ne!(game.objs.aliens[object as usize].flags & AFONFIRE, 0);

    let (mut game, object) = setup();
    starbull_istrat(&mut game, object);
    game.objs.aliens[object as usize].hp = STARBULL_DAMAGE_SMOKE_HP;
    assert_cadence(&mut game, object, starbull_strat, EVERY_FOURTH_FRAME_PERIOD);
    assert_ne!(game.objs.aliens[object as usize].flags & AFONFIRE, 0);

    let (mut healthy_game, healthy) = setup();
    starbull_istrat(&mut healthy_game, healthy);
    healthy_game.objs.aliens[healthy as usize].hp = STARBULL_DAMAGE_SMOKE_HP + 1;
    healthy_game.vars.gameframe = 0;
    starbull_strat(&mut healthy_game, healthy);
    assert_eq!(smoke_count(&healthy_game), 0);
    assert_eq!(
        healthy_game.objs.aliens[healthy as usize].flags & AFONFIRE,
        0,
    );
}

#[derive(Clone)]
struct SoundLog(Rc<RefCell<Vec<u8>>>);

impl Hooks for SoundLog {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

#[test]
fn damaged_cruiser_uses_both_authored_destruction_sound_frames() {
    let sounds = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(SoundLog(sounds.clone())));
    let object = game.objs.alloc().expect("cruiser");

    game.vars.gameframe = 7;
    cruiser2fire_strat(&mut game, object);
    game.vars.gameframe = 14;
    cruiser2fire_strat(&mut game, object);

    assert_eq!(
        *sounds.borrow(),
        vec![
            CRUISER_NEAR_DESTRUCTION_SOUND,
            CRUISER_MID_DESTRUCTION_SOUND,
        ],
    );
}
