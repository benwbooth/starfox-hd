//! enemy_a `angle_xz` == ROM `yanglexy` (i16 wrapping deltas).

use sf_core::aim_angle::yanglexy;
use sf_game::alien::Alien;
use sf_strat::common::strat_angle_xz;

fn alien_at(x: i16, y: i16, z: i16) -> Alien {
    let mut a = Alien::default();
    a.worldx = x;
    a.worldy = y;
    a.worldz = z;
    a
}

#[test]
fn enemy_a_angle_xz_matches_yanglexy_and_common() {
    let src = alien_at(100, 0, 200);
    let dst = alien_at(500, 50, -300);
    let dx = dst.worldx.wrapping_sub(src.worldx);
    let dz = dst.worldz.wrapping_sub(src.worldz);
    let y = yanglexy(dx, dz);
    assert_eq!(strat_angle_xz(&src, &dst), y);
    // Cardinal: +X → 64
    assert_eq!(
        yanglexy(
            alien_at(1000, 0, 0).worldx.wrapping_sub(0),
            alien_at(1000, 0, 0).worldz.wrapping_sub(0)
        ),
        64
    );
}

#[test]
fn wrapping_deltas_match_rom_not_i32_promotion() {
    // |dx| > 32767: i16 wrapping ≠ i32 promotion.
    let src = alien_at(-20000, 0, 0);
    let dst = alien_at(20000, 0, 1000);
    let wrap = yanglexy(
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    );
    let promoted = {
        let dx = (dst.worldx as i32 - src.worldx as i32) as f32;
        let dz = (dst.worldz as i32 - src.worldz as i32) as f32;
        let mut a = dx.atan2(dz);
        if a < 0.0 {
            a += 2.0 * 3.141_592_65;
        }
        ((a * (256.0 / (2.0 * 3.141_592_65))) as i32) as u8
    };
    assert_ne!(wrap, promoted, "fixture must exercise wrap≠promote");
    assert_eq!(strat_angle_xz(&src, &dst), wrap);
}
