//! Path spawn Roffs = ROM `strat_roffs_full_scaled` (no float sin/cos).

use sf_core::snes_trig::{rotate_8xz, rotate_8yx, rotate_8yz, strat_roffs_full_scaled};

#[test]
fn path_spawn_scale2_matches_rotate8_chain() {
    // P_SPAWN: coord/4 payload, then ASL×2 after rotate_8yx→yz→xz.
    let rotz = 32u8;
    let rotx = 16u8;
    let roty = 64u8;
    let (ox, oy, oz) = (10i8, -5i8, 20i8);
    let got = strat_roffs_full_scaled(rotz, rotx, roty, ox, oy, oz, 2);

    let (x1, y1) = rotate_8yx(rotz, ox, oy);
    let (y2, z2) = rotate_8yz(rotx, y1 as i8, oz);
    let (x3, z3) = rotate_8xz(roty, x1 as i8, z2 as i8);
    assert_eq!(got, (x3 << 2, y2 << 2, z3 << 2));
}

#[test]
fn path_child_scale0_matches_unscaled_chain() {
    // COSTAB[0]=127 attenuates magnitude — not a pure copy of the offset.
    let (ox, oy, oz) = (7i8, -3i8, 11i8);
    let got = strat_roffs_full_scaled(0, 0, 0, ox, oy, oz, 0);
    let (x1, y1) = rotate_8yx(0, ox, oy);
    let (y2, z2) = rotate_8yz(0, y1 as i8, oz);
    let (x3, z3) = rotate_8xz(0, x1 as i8, z2 as i8);
    assert_eq!(got, (x3, y2, z3));
    assert_ne!(got, (ox as i16, oy as i16, oz as i16));
}

#[test]
fn yaw90_full_roffs_matches_staged_xz() {
    // Identity pitch/roll still runs yz (COSTAB attenuates Z) before xz yaw.
    let (x, y, z) = strat_roffs_full_scaled(0, 0, 64, 0, 0, 40, 0);
    assert_eq!(y, 0);
    let (x1, y1) = rotate_8yx(0, 0, 0);
    let (y2, z2) = rotate_8yz(0, y1 as i8, 40);
    let (rx, rz) = rotate_8xz(64, x1 as i8, z2 as i8);
    assert_eq!((x, y, z), (rx, y2, rz));
    assert!(x < 0, "yaw 90° must pull +Z toward −X");
}
