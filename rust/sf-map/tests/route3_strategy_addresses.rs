use sf_map::catalog::{get_map_data, map_id};
use sf_map::consts::{op, DirectStrategy, STRATEGY_SLOWMETEOR};

fn first_slowmeteor_spawn(strategy: DirectStrategy) -> [u8; 14] {
    let mut out = [0; 14];
    out[0] = op::DIRECTOBJ;
    out[1..3].copy_from_slice(&500u16.to_le_bytes());
    out[3..5].copy_from_slice(&400i16.to_le_bytes());
    out[5..7].copy_from_slice(&(-160i16).to_le_bytes());
    out[7..9].copy_from_slice(&4000i16.to_le_bytes());
    out[9..11].copy_from_slice(&275u16.to_le_bytes());
    out[11..13].copy_from_slice(&(strategy.id() as u16).to_le_bytes());
    out[13] = 0;
    out
}

fn compatibility_spawn(encoded: u32) -> [u8; 14] {
    let mut out = first_slowmeteor_spawn(STRATEGY_SLOWMETEOR);
    out[0] = op::NORMOBJ;
    out[11..13].copy_from_slice(&(encoded as u16).to_le_bytes());
    out[13] = (encoded >> 16) as u8;
    out
}

#[test]
fn asteroid_belt_uses_the_slowmeteor_strategy_not_player_exitbase() {
    let level = get_map_data(map_id::M3_2).expect("route 3 asteroid belt map");
    let expected = first_slowmeteor_spawn(STRATEGY_SLOWMETEOR);
    assert!(
        level
            .data
            .windows(expected.len())
            .any(|bytes| bytes == expected),
        "the first asteroid1 spawn must use the typed slowmeteor strategy"
    );

    // 0x030003 belongs to the player exit-base callback.  This stale address
    // made culled asteroids enter playerdead_Istrat and restart the stage.
    let stale = compatibility_spawn(0x03_0003);
    assert!(
        !level.data.windows(stale.len()).any(|bytes| bytes == stale),
        "asteroid1 must never be encoded as the player exit-base strategy"
    );
}
