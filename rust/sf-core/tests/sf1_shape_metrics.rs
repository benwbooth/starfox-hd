use sf_core::sf1_shape_metrics::sf1_shape_metrics;

const NULL_SHAPE: u16 = 0;
const PLAYER_SHIP_SHAPE: u16 = 2;
const MEDIUM_EXPLOSION_SPRITE_SHAPE: u16 = 462;
const NULL_VISUAL_EXTENT: u16 = 0;
const PLAYER_VISUAL_EXTENT: u16 = 80;
const MEDIUM_EXPLOSION_VISUAL_EXTENT: u16 = 64;
const UNSHIFTED_COORDINATES: u8 = 0;
const MEDIUM_EXPLOSION_COORDINATE_SHIFT: u8 = 4;

#[test]
fn generated_metrics_retain_the_source_explosion_inputs() {
    let null = sf1_shape_metrics(NULL_SHAPE).expect("nullshape header");
    assert_eq!(null.visual_extent, NULL_VISUAL_EXTENT);
    assert_eq!(null.coordinate_shift, UNSHIFTED_COORDINATES);

    let player = sf1_shape_metrics(PLAYER_SHIP_SHAPE).expect("player ShapeHdr");
    assert_eq!(player.visual_extent, PLAYER_VISUAL_EXTENT);
    assert_eq!(player.coordinate_shift, UNSHIFTED_COORDINATES);

    let sprite = sf1_shape_metrics(MEDIUM_EXPLOSION_SPRITE_SHAPE).expect("explosion2 ShapeHdr");
    assert_eq!(sprite.visual_extent, MEDIUM_EXPLOSION_VISUAL_EXTENT);
    assert_eq!(sprite.coordinate_shift, MEDIUM_EXPLOSION_COORDINATE_SHIFT);
}
