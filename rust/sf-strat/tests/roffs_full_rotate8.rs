//! Tick 173–179: `s_add_Roffs2pos` full / yaw / roll / pitch / non-uniform → `rotate_8*`.

use sf_strat::snes_trig::{
    rotate_8xz, rotate_8yx, rotate_8yz, strat_roffs_full, strat_roffs_full_i16,
    strat_roffs_full_scaled, strat_roffs_pitch_yaw, strat_roffs_roll, strat_roffs_roll_scaled,
    strat_roffs_roll_yaw, strat_roffs_yaw, strat_roffs_yaw_i16, strat_roffs_yaw_scaled,
    strat_roffs_yaw_scaled_xyz,
};

/// rotz=180 flips offy sign (boss2 inverted-top muzzle).
#[test]
fn roffs_full_rotz180_flips_offy() {
    let (x, y, z) = strat_roffs_full(128, 0, 0, 0, -60, 0);
    assert_eq!(x, 0);
    assert_eq!(z, 0);
    // rotate_8yx(180): cos≈-1 → y' = -(-60) = +60 (mulslog may lose 1).
    assert!(y > 0, "rotz=180 must flip -60 offy upward; got y={y}");
    assert!(y >= 50, "expected ~+60, got {y}");
}

/// Pre-scaled i16 (-59<<2) undoes scale then re-applies.
#[test]
fn roffs_full_i16_undoes_post_scale() {
    let direct = strat_roffs_full_scaled(0, 0, 0, 0, -59, 0, 2);
    let folded = strat_roffs_full_i16(0, 0, 0, 0, -236, 0);
    assert_eq!(folded, direct);
}

/// Chain matches composing the three rotate_8 leaves (identity pitch/yaw).
#[test]
fn roffs_full_matches_rotate8_chain() {
    let rotz = 40u8;
    let (x1, y1) = rotate_8yx(rotz, 10, -20);
    let (y2, z2) = rotate_8yz(0, y1 as i8, 5);
    let (x3, z3) = rotate_8xz(0, x1 as i8, z2 as i8);
    let got = strat_roffs_full(rotz, 0, 0, 10, -20, 5);
    assert_eq!(got, (x3, y2, z3));
}

/// Yaw-only flags 0,1,0: Y unrotated; matches `rotate_8xz` (nega inside).
#[test]
fn roffs_yaw_matches_rotate8xz() {
    let roty = 40u8;
    let (rx, rz) = rotate_8xz(roty, 10, 30);
    let got = strat_roffs_yaw(roty, 10, -50, 30);
    assert_eq!(got, (rx, -50, rz));
}

/// Yaw 180 flips +Z toward −Z (bossA / boss2 yaw-only child attach).
#[test]
fn roffs_yaw180_flips_plus_z() {
    let (x, y, z) = strat_roffs_yaw(128, 0, 0, 60);
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    assert!(z < 0, "yaw=180 must flip +60 offz; got z={z}");
    assert!(z <= -50, "expected ~-60, got {z}");
}

/// Pre-scaled i16 yaw offs undo max clean ASL then re-apply (mulslog ≠ exact ×).
#[test]
fn roffs_yaw_i16_undoes_post_scale() {
    let direct = strat_roffs_yaw_scaled(0, -85, -50, 0, 2);
    let folded = strat_roffs_yaw_i16(0, -85i16 << 2, -50i16 << 2, 0);
    assert_eq!(folded, direct);
    // Y unrotated → exact −200; X via mulslog(−85, cos0=127) <<2 → −336.
    assert_eq!(folded.1, -200);
    assert_eq!(folded.2, 0);
    assert_eq!(folded.0, -336);
}

/// Flags 0,0,1 (helpball): Z/roll only — matches `rotate_8yx`, Z unrotated.
#[test]
fn roffs_roll_matches_rotate8yx() {
    let rotz = 40u8;
    let (x, y) = rotate_8yx(rotz, 0, 30);
    let got = strat_roffs_roll(rotz, 0, 30, 60);
    assert_eq!(got, (x, y, 60));
}

/// Flags 0,1,1 (bossFCsmoke): roll then yaw; skip pitch.
#[test]
fn roffs_roll_yaw_chain() {
    let rotz = 40u8;
    let roty = 80u8;
    let (x1, y1) = rotate_8yx(rotz, 0, -20);
    let (x3, z3) = rotate_8xz(roty, x1 as i8, 0);
    let got = strat_roffs_roll_yaw(rotz, roty, 0, -20, 0);
    assert_eq!(got, (x3, y1, z3));
}

/// boss2plasma: yaw then ASL x<<2 / y<<0 / z<<4 (not pre-shift z then rotate).
#[test]
fn roffs_yaw_nonuniform_scales_2_0_4() {
    let roty = 64u8; // 90° — mixes +Z into ±X via rotate_8xz(nega)
    let dist = 10i8;
    let (rx, ry, rz) = strat_roffs_yaw(roty, 0, 0, dist);
    let got = strat_roffs_yaw_scaled_xyz(roty, 0, 0, dist, 2, 0, 4);
    assert_eq!(got, (rx << 2, ry, rz << 4));
    // Pre-shifting z by 4 then uniform yaw would disagree once X is nonzero.
    let wrong = strat_roffs_yaw_i16(roty, 0, 0, (dist as i16) << 4);
    assert_ne!(
        got.0, wrong.0,
        "non-uniform x scale must differ from pre<<4"
    );
}

/// Flags 1,1,0 (updateengine): pitch then yaw; skip roll.
#[test]
fn roffs_pitch_yaw_chain() {
    let rotx = 40u8;
    let roty = 80u8;
    let (y2, z2) = rotate_8yz(rotx, 0, -40);
    let (x3, z3) = rotate_8xz(roty, 0, z2 as i8);
    let got = strat_roffs_pitch_yaw(rotx, roty, 0, 0, -40);
    assert_eq!(got, (x3, y2, z3));
}

/// Identity pitch/yaw: engine flame sits ~−40 on Z (mulslog may lose 1).
#[test]
fn roffs_pitch_yaw_identity_keeps_neg_z() {
    let (x, y, z) = strat_roffs_pitch_yaw(0, 0, 0, 0, -40);
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    assert!((-42..=-38).contains(&z), "expected ~-40 behind, z={z}");
}

/// spacepilonP: roll then ASL ×3 (flags 0,0,1 + scales 3,3,3).
#[test]
fn roffs_roll_scaled_spacepilon() {
    let rotz = 0u8;
    let rel = -62i8; // -500/8
    let (x, y, z) = strat_roffs_roll_scaled(rotz, 0, rel, 0, 3);
    let (bx, by, bz) = strat_roffs_roll(rotz, 0, rel, 0);
    assert_eq!((x, y, z), (bx << 3, by << 3, bz << 3));
    assert_eq!(x, 0);
    assert_eq!(z, 0);
    // mulslog(−62, cos0=127) <<3 ≈ −488 (not exact −496).
    assert!(
        (-500..=-480).contains(&y),
        "expected ~-488 after <<3, y={y}"
    );
}
