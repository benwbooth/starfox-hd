//! Cross-crate asset contract: strategy expectations alone once blessed the
//! wrong numeric id, drawing the oversized fire/burn-mark image as smoke.
use sf_core::sf1_shape_metrics::sf1_shape_metrics;
use sf_game::Game;
use sf_render::shape_data::{SHAPE_DATA, SHAPE_EXT_FIRE, SHAPE_EXT_SMOKE};

const SOURCE_SMOKE_EXTENT: u16 = 40;
const SOURCE_SMOKE_COORDINATE_SHIFT: u8 = 2;

#[test]
fn emitted_smoke_uses_the_smoke_asset_header_and_color_animation() {
    let mut game = Game::new();
    let parent = game.objs.alloc().expect("smoke emitter");
    let smoke = sf_strat::common::makesmoke_srou(&mut game, parent).expect("smoke object");
    let shape_id = game.objs.aliens[usize::from(smoke)].shape;
    let asset = SHAPE_DATA
        .iter()
        .find(|asset| asset.shape_id == shape_id)
        .unwrap();
    assert_eq!(
        asset.name, "smoke",
        "smoke must not use the fire/burn-mark bitmap"
    );
    assert_eq!(asset.default_color_table, "smoke_c");
    assert_eq!(shape_id, SHAPE_EXT_SMOKE);
    assert_ne!(shape_id, SHAPE_EXT_FIRE);
    let metrics = sf1_shape_metrics(shape_id).unwrap();
    // USHAPES.ASM firesize=10, shift=2. Also verified directly in the
    // retail Rev-2 smoke header: all extents and size are 40, not 188.
    assert_eq!(metrics.visual_extent, SOURCE_SMOKE_EXTENT);
    assert_eq!(metrics.half_extents, [SOURCE_SMOKE_EXTENT as i16; 3]);
    assert_eq!(metrics.coordinate_shift, SOURCE_SMOKE_COORDINATE_SHIFT);
}
