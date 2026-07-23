//! Unit tests for `msh_rot_points16` (same FMULT math as `wmat_rot_point`).

use sf_strat::snes_trig::{msh_rot_points16, wmat_rot_point};

const ONE: i16 = 32766;

#[test]
fn msh_rot_points16_matches_single() {
    let mat = [[ONE, 0, 0], [0, ONE, 0], [0, 0, ONE]];
    let mut pts = [(100i16, -50, 200), (0, 0, 0), (-1000, 500, 300)];
    let expected: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| wmat_rot_point(mat, x, y, z))
        .collect();
    msh_rot_points16(mat, &mut pts);
    assert_eq!(pts.as_slice(), expected.as_slice());
}

#[test]
fn msh_rot_points16_dense_matrix() {
    let mat = [
        [1000, -2000, 3000],
        [-4000, 5000, -6000],
        [7000, -8000, 9000],
    ];
    let mut pts = [(1i16, 2, 3), (-10, 20, -30), (100, -200, 300)];
    let expected: Vec<_> = pts
        .iter()
        .map(|&(x, y, z)| wmat_rot_point(mat, x, y, z))
        .collect();
    msh_rot_points16(mat, &mut pts);
    assert_eq!(pts.as_slice(), expected.as_slice());
}

#[test]
fn msh_rot_points_x16_emits_mirrored_pair() {
    use sf_strat::snes_trig::msh_rot_points_x16;
    let mat = [[ONE, 0, 0], [0, ONE, 0], [0, 0, ONE]];
    let pts = [(100i16, 20, 30)];
    let mut out = Vec::new();
    msh_rot_points_x16(mat, &pts, &mut out);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], wmat_rot_point(mat, 100, 20, 30));
    assert_eq!(out[1], wmat_rot_point(mat, -100, 20, 30));
}
