use sf_core::scene::{DepthColors, DepthThresholds, GamePalette, SceneStyle};
use sf_game::game::Game;
use sf_game::vars::GameVars;
use sf_map::builder::MapBuilder;
use sf_map::consts::cb;

#[test]
fn background_profiles_follow_the_source_scene_operations() {
    let mut world = GameVars::init();
    world.set_scene_style_for_bg(4);
    assert_eq!(
        world.scene_style,
        SceneStyle {
            game_palette: GamePalette::Blue,
            depth_colors: DepthColors::Night,
            depth_thresholds: DepthThresholds::StageOne,
            shadow_height: 0,
        }
    );
    assert_eq!(world.palfade_target, None);

    world.set_scene_style_for_bg(10);
    assert_eq!(world.scene_style.shadow_height, 400);
    world.set_scene_style_for_bg(11);
    assert_eq!(world.scene_style.shadow_height, 400);

    world.set_scene_style_for_bg(23);
    assert_eq!(world.scene_style.game_palette, GamePalette::Blue);
    assert_eq!(world.scene_style.depth_colors, DepthColors::Mist);
    assert_eq!(world.scene_style.depth_thresholds, DepthThresholds::Mist);
}

#[test]
fn titania_fog_clear_callback_installs_red_normal_scene() {
    let mut builder = MapBuilder::new();
    builder.mapcodejsl_builtin(cb::BG_1_4B_1_L);
    builder.mapend(0);
    let (data, labels) = builder.finish();
    let level = sf_map::levels::BuiltLevel {
        data,
        labels,
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    };

    let mut game = Game::new();
    game.load_level(&level);
    game.world.register_named_callbacks(
        &[(cb::BG_1_4B_1_L, "level2_3_bg_1_4b_1")],
        &[],
        &level.labels,
    );
    game.vars.set_scene_style_for_bg(23);
    game.map_exec();

    assert_eq!(
        game.vars.scene_style,
        SceneStyle {
            game_palette: GamePalette::Red,
            depth_colors: DepthColors::Red,
            depth_thresholds: DepthThresholds::Normal,
            shadow_height: 0,
        }
    );
    assert_eq!(game.vars.palfade_target, None);
}
