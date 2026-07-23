//! ROM `rotproj_l` local offset leaf (SPRITES.ASM:183) via `rotate_8*`.

use sf_strat::snes_trig::{rotate_8xz, rotate_8yz, rotproj_local_offset};

#[test]
fn rotproj_identity_angles_is_neg_z() {
    // Two rotate_8 passes shrink |z| via mulslog_mac8: -30 → -29 → -28.
    let (x, y, z) = rotproj_local_offset(0, 0);
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    assert_eq!(z, -28);
}

#[test]
fn rotproj_matches_manual_rotate8_chain() {
    for rotx in (0..=255u8).step_by(17) {
        for roty in (0..=255u8).step_by(19) {
            let (y1, z1) = rotate_8yz(rotx, 0, -30);
            let (x2, z2) = rotate_8xz(roty, 0, z1 as i8);
            assert_eq!(rotproj_local_offset(rotx, roty), (x2, y1, z2));
        }
    }
}

#[test]
fn mcore_zrot_adds_vz_asr3() {
    use sf_strat::snes_trig::mcore_zrot;
    let mut r = 10u8;
    mcore_zrot(&mut r, 80); // 80>>3 = 10
    assert_eq!(r, 20);
    mcore_zrot(&mut r, -16); // -16>>3 = -2
    assert_eq!(r, 18);
}
