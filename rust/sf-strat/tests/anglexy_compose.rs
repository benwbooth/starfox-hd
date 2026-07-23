//! Compose helpers for ROM ANGLEXY_* / XANGLE* / YANGLE* (STRATROU.ASM).
//!
//! These wrap the shared `sf_core::aim_angle` f32 atan2→u8 path. Full GSU
//! arctan16 bit-exactness is still ±1 LSB off-axis (`gsu_arctan.rs`).

use sf_core::aim_angle::{xanglexabs, xanglexy, xzdiffs, xzdiffs_abs_manhattan};
use sf_game::alien::Alien;
use sf_strat::common::{
    strat_angle_xz, strat_angle_xz_abs, strat_angle_y_abs, strat_angle_yz, strat_angle_yz_abs,
    strat_dist_xz,
};

fn alien_at(x: i16, y: i16, z: i16) -> Alien {
    let mut a = Alien::default();
    a.worldx = x;
    a.worldy = y;
    a.worldz = z;
    a
}

#[test]
fn angle_xz_abs_matches_obj_form() {
    let src = alien_at(100, 0, 200);
    let dst = alien_at(500, 0, -300);
    assert_eq!(
        strat_angle_xz(&src, &dst),
        strat_angle_xz_abs(&src, dst.worldx, dst.worldz)
    );
    assert_eq!(
        strat_angle_xz_abs(&src, dst.worldx, dst.worldz),
        strat_angle_y_abs(&src, dst.worldx, dst.worldz)
    );
}

#[test]
fn angle_yz_uses_dist_xz_as_adjacent() {
    let src = alien_at(0, 0, 0);
    let dst = alien_at(300, 150, 400);
    let a = strat_angle_yz(&src, &dst);
    let expect = xanglexy(150, 300, 400);
    assert_eq!(a, expect);
    assert_eq!(strat_dist_xz(&src, &dst), xzdiffs(300, 400));
}

#[test]
fn angle_yz_abs_uses_manhattan_not_scaled() {
    // ROM Xanglexabs_l adjacent = xzdiffs_abs Manhattan; Xanglexy_l uses
    // scaled Euclid — they diverge when |dx| != |dz|.
    let src = alien_at(10, 20, 30);
    let dst = alien_at(-100, 80, 250);
    let dx = dst.worldx.wrapping_sub(src.worldx);
    let dy = dst.worldy.wrapping_sub(src.worldy);
    let dz = dst.worldz.wrapping_sub(src.worldz);
    let man = xzdiffs_abs_manhattan(dx, dz);
    let sc = xzdiffs(dx, dz);
    assert_ne!(man, sc, "fixture must exercise Manhattan≠scaled");
    assert_eq!(
        strat_angle_yz_abs(&src, dst.worldx, dst.worldy, dst.worldz),
        xanglexabs(dy, dx, dz)
    );
    assert_eq!(strat_angle_yz(&src, &dst), xanglexy(dy, dx, dz));
    assert_ne!(
        strat_angle_yz(&src, &dst),
        strat_angle_yz_abs(&src, dst.worldx, dst.worldy, dst.worldz)
    );
}

#[test]
fn cardinal_yaw_matches_known_octants() {
    let src = alien_at(0, 0, 0);
    // atan2(dx, dz): +Z → 0, +X → 64, -Z → 128, -X → 192
    assert_eq!(strat_angle_xz(&src, &alien_at(0, 0, 1000)), 0);
    assert_eq!(strat_angle_xz(&src, &alien_at(1000, 0, 0)), 64);
    assert_eq!(strat_angle_xz(&src, &alien_at(0, 0, -1000)), 128);
    assert_eq!(strat_angle_xz(&src, &alien_at(-1000, 0, 0)), 192);
}
