use sf_map::{builder::MapBuilder, consts};

#[test]
fn eligible_source_object_uses_quantized_coordinates() {
    let mut builder = MapBuilder::new();
    builder.mapobj(
        0,
        100,
        -90,
        1_400,
        consts::sh::FRIENDSHIP_4,
        consts::is::PATHDHA,
    );

    let (bytes, _) = builder.finish();
    assert_eq!(
        bytes,
        vec![
            consts::op::QOBJ,
            0,
            25,
            233,
            87,
            consts::sh::FRIENDSHIP_4 as u8,
            consts::is::PATHDHA as u8,
        ]
    );
}

#[test]
fn matching_strategy_shape_uses_shape_implied_quantized_object() {
    let mut builder = MapBuilder::new();
    builder.mapobj(
        0,
        100,
        -88,
        1_400,
        consts::sh::MYSHIP_4,
        consts::is::FRIENDEXITBASE,
    );

    let (bytes, _) = builder.finish();
    assert_eq!(
        bytes,
        vec![
            consts::op::QOBJ2,
            0,
            25,
            234,
            87,
            consts::is::FRIENDEXITBASE as u8,
        ]
    );
}

#[test]
fn out_of_range_source_object_keeps_full_coordinates() {
    let mut builder = MapBuilder::new();
    builder.mapobj(
        0,
        3_000,
        3_000,
        3_000,
        consts::sh::FRIENDSHIP_4,
        consts::is::PATHDHA,
    );

    let (bytes, _) = builder.finish();
    assert_eq!(bytes[0], consts::op::MAPOBJ);
    assert_eq!(i16::from_le_bytes([bytes[3], bytes[4]]), 3_000);
    assert_eq!(i16::from_le_bytes([bytes[5], bytes[6]]), 3_000);
    assert_eq!(i16::from_le_bytes([bytes[7], bytes[8]]), 3_000);
}

#[test]
fn negative_source_depth_uses_full_shape_implied_object() {
    let mut builder = MapBuilder::new();
    builder.mapobj(
        0,
        -216,
        -312,
        -200,
        consts::sh::MYSHIP_4,
        consts::is::FRIENDEXITBASE,
    );

    let (bytes, _) = builder.finish();
    assert_eq!(bytes[0], consts::op::DOBJ);
    assert_eq!(i16::from_le_bytes([bytes[7], bytes[8]]), -200);
}
