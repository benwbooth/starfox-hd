use sf_game::alien::{ASF_COLLDISABLE, ATZREMOVE};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_game::world::op;
use sf_map::levels::BuiltLevel;
use sf_strat::table;

const IS_TADPOLE: u8 = 227;
const IS_BOSS7INTRO: usize = 239;
const SH_TADPOLE: u16 = 227;
const SH_DEBOSS_1: u16 = 312;
const SH_DEBOSS_0: u16 = 440;
const SH_DEBOSS_2: u16 = 441;

fn level(data: Vec<u8>) -> BuiltLevel {
    BuiltLevel {
        data,
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}

#[test]
fn dynamic_map_object_uses_generated_strategy_shape_and_native_callback() {
    let mut data = vec![op::DOBJ];
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&100i16.to_le_bytes());
    data.extend_from_slice(&200i16.to_le_bytes());
    data.extend_from_slice(&300i16.to_le_bytes());
    data.push(IS_TADPOLE);
    data.push(op::WAIT);
    data.extend_from_slice(&1_000u16.to_le_bytes());

    let mut game = Game::new();
    table::register_all(&mut game);
    game.load_level(&level(data));
    game.map_exec();

    let object = &game.objs.aliens[0];
    assert_eq!(object.shape, SH_TADPOLE);
    assert!(object.stratptr.is_some(), "DOBJ strategy row resolves");
}

#[test]
fn intro_commander_builds_the_complete_three_piece_model() {
    let mut game = Game::new();
    table::register_all(&mut game);
    let parent = game.objs.alloc().expect("intro commander parent");
    strat_init_obj_vars(&mut game.objs.aliens[parent as usize]);
    game.objs.aliens[parent as usize].shape = SH_DEBOSS_1;
    game.objs.aliens[parent as usize].type_ |= ATZREMOVE;

    let init = game.world.istrats[IS_BOSS7INTRO].expect("boss7intro row");
    game.call_strat(init, parent);

    let parent_object = &game.objs.aliens[parent as usize];
    assert_ne!(parent_object.sflags & ASF_COLLDISABLE, 0);
    assert_eq!(parent_object.type_ & ATZREMOVE, 0);
    assert_eq!(
        (parent_object.vx, parent_object.vy, parent_object.vz),
        (0, 5, 15)
    );

    let mut shapes: Vec<u16> = game
        .objs
        .aliens
        .iter()
        .filter(|object| object.active)
        .map(|object| object.shape)
        .collect();
    shapes.sort_unstable();
    assert_eq!(shapes, vec![SH_DEBOSS_1, SH_DEBOSS_0, SH_DEBOSS_2]);
}
