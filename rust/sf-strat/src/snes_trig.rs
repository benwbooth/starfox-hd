//! Bit-exact ports of the ROM's fixed-point trig + multiply (STRATROU.ASM /
//! MACROS.INC). Shared tables / `mulslog` / `rotate_8*` / `rotate_16*` /
//! `strat_roffs_full*` / `strat_roffs_roll` live in [`sf_core::snes_trig`];
//! this module re-exports them and keeps the remaining `strat_roffs_*`
//! flag variants + GSU matrix helpers.
//! Verified against the real ROM by `sf-oracle` (tests/gen_3dvecs.rs).

pub use sf_core::snes_trig::{
    mulslog, mulslog_mac8, rotate_16xz, rotate_16yz, rotate_8xz, rotate_8yx, rotate_8yz,
    strat_roffs_full, strat_roffs_full_scaled, strat_roffs_roll, COSTAB, SINTAB,
};

/// GSU `FMULT` + `ROL`: signed `(a * b) >> 15` truncated to i16.
/// Matches Super FX `mdotprod16mq` per-term (MMACS.MC:787).
#[inline]
pub fn gsu_fmult_asr15(a: i16, b: i16) -> i16 {
    (((a as i32) * (b as i32)) >> 15) as i16
}

/// ROM `mwmatrotp16` (MWROT.MC:19) / `wmatrotp16_l` (OBJ.ASM:141).
///
/// Rotate a 16-bit world point by the 3×3 world matrix in GSU RAM
/// (`m_wmat11..33`, Q15). Output is `m_bigx/y/z`:
/// ```text
/// bigx = (x*m11 + y*m21 + z*m31) >> 15   (per-term >>15, then 16-bit add)
/// bigy = (x*m12 + y*m22 + z*m32) >> 15
/// bigz = (x*m13 + y*m23 + z*m33) >> 15
/// ```
/// `mat` is row-major: `mat[r][c]` = `m_wmat{r+1}{c+1}`.
#[inline]
pub fn wmat_rot_point(mat: [[i16; 3]; 3], x: i16, y: i16, z: i16) -> (i16, i16, i16) {
    let dot = |c0: i16, c1: i16, c2: i16| {
        gsu_fmult_asr15(x, c0)
            .wrapping_add(gsu_fmult_asr15(y, c1))
            .wrapping_add(gsu_fmult_asr15(z, c2))
    };
    (
        dot(mat[0][0], mat[1][0], mat[2][0]),
        dot(mat[0][1], mat[1][1], mat[2][1]),
        dot(mat[0][2], mat[1][2], mat[2][2]),
    )
}

/// ROM `msh_rotpoints16` (MOBJ.MC:1319): rotate `points` by object matrix
/// `m_mat*` using the same FMULT>>15 per-term sum as `wmat_rot_point`.
/// In-place: each `(x,y,z)` triple is replaced by the rotated result.
#[inline]
pub fn msh_rot_points16(mat: [[i16; 3]; 3], points: &mut [(i16, i16, i16)]) {
    for p in points.iter_mut() {
        *p = wmat_rot_point(mat, p.0, p.1, p.2);
    }
}

/// ROM `msh_rotpointsx16` (MOBJ.MC:1419) / `mdotprod16mqx`: for each input
/// point, emit the pair with `±x` in the first matrix column contribution
/// (mirrored geometry). Output length is `2 * points.len()`.
#[inline]
pub fn msh_rot_points_x16(
    mat: [[i16; 3]; 3],
    points: &[(i16, i16, i16)],
    out: &mut Vec<(i16, i16, i16)>,
) {
    out.clear();
    for &(x, y, z) in points {
        out.push(wmat_rot_point(mat, x, y, z));
        out.push(wmat_rot_point(mat, x.wrapping_neg(), y, z));
    }
}

/// ROM `rotproj_l` local offset (SPRITES.ASM:183): start at `(0,0,-30)`,
/// pitch with `rotate_8yz(rotx)`, then yaw with `rotate_8xz(roty)`.
/// Returns the signed-byte world offset before view-subtract + `wmatrotp16`.
#[inline]
pub fn rotproj_local_offset(rotx: u8, roty: u8) -> (i16, i16, i16) {
    let (y1, z1) = rotate_8yz(rotx, 0, -30);
    let (x2, z2) = rotate_8xz(roty, 0, z1 as i8);
    // rotate_8yz wrote y into y2; x stays 0 until xz rotate (x1 was 0).
    // After xz: x2 from rotate_8xz; y is unchanged from y1 (ROM copies y2→y1
    // before xz, and xz does not touch y).
    (x2, y1, z2)
}

/// Convenience for call sites that pass i16 offsets. When all axes fit in i8,
/// rotates as ROM byte loads. When any axis exceeds i8, undoes the smallest
/// uniform `<<scale` that restores a byte triple (ROM "use BYTE and SHIFT"),
/// then re-applies the scale after `rotate_8*`.
#[inline]
pub fn strat_roffs_full_i16(
    rotz: u8,
    rotx: u8,
    roty: u8,
    offx: i16,
    offy: i16,
    offz: i16,
) -> (i16, i16, i16) {
    let fits = |v: i16| (-128..=127).contains(&v);
    if fits(offx) && fits(offy) && fits(offz) {
        return strat_roffs_full(rotz, rotx, roty, offx as i8, offy as i8, offz as i8);
    }
    for scale in (1u32..=4).rev() {
        let sx = offx >> scale;
        let sy = offy >> scale;
        let sz = offz >> scale;
        if (sx << scale) == offx
            && (sy << scale) == offy
            && (sz << scale) == offz
            && fits(sx)
            && fits(sy)
            && fits(sz)
        {
            return strat_roffs_full_scaled(rotz, rotx, roty, sx as i8, sy as i8, sz as i8, scale);
        }
    }
    strat_roffs_full(rotz, rotx, roty, offx as i8, offy as i8, offz as i8)
}

/// `strat_roffs_roll` then uniform `ASL` post-scale (e.g. spacepilonP `3,3,3`).
#[inline]
pub fn strat_roffs_roll_scaled(
    rotz: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_roll(rotz, offx, offy, offz);
    (x << scale, y << scale, z << scale)
}

/// ROM `s_add_Roffs2pos` B-mode flags 1,1,0 — pitch then yaw (`rotate_8yz`
/// then `rotate_8xz`); skip roll. Used by `updateengine` / `boost_strat`.
#[inline]
pub fn strat_roffs_pitch_yaw(rotx: u8, roty: u8, offx: i8, offy: i8, offz: i8) -> (i16, i16, i16) {
    let (y2, z2) = rotate_8yz(rotx, offy, offz);
    let (x3, z3) = rotate_8xz(roty, offx, z2 as i8);
    (x3, y2, z3)
}

/// `strat_roffs_pitch_yaw` then uniform `ASL` post-scale.
#[inline]
pub fn strat_roffs_pitch_yaw_scaled(
    rotx: u8,
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_pitch_yaw(rotx, roty, offx, offy, offz);
    (x << scale, y << scale, z << scale)
}

/// ROM `s_add_Roffs2pos` B-mode flags 0,1,1 — roll then yaw (`rotate_8yx`
/// then `rotate_8xz`); skip pitch. Macro order: `\A` rotz, then `\9` roty.
/// Used by bossFCsmoke / `dobossrot*_srou` (`#0,#-20,#0`).
#[inline]
pub fn strat_roffs_roll_yaw(rotz: u8, roty: u8, offx: i8, offy: i8, offz: i8) -> (i16, i16, i16) {
    let (x1, y1) = rotate_8yx(rotz, offx, offy);
    let (x3, z3) = rotate_8xz(roty, x1 as i8, offz);
    (x3, y1, z3)
}

/// `strat_roffs_roll_yaw` then uniform `ASL` post-scale.
#[inline]
pub fn strat_roffs_roll_yaw_scaled(
    rotz: u8,
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_roll_yaw(rotz, roty, offx, offy, offz);
    (x << scale, y << scale, z << scale)
}

/// ROM `s_add_Roffs2pos` B-mode flags 0,1,0 — yaw only via `rotate_8xz_l`
/// (angle negated inside). Y is not rotated.
#[inline]
pub fn strat_roffs_yaw(roty: u8, offx: i8, offy: i8, offz: i8) -> (i16, i16, i16) {
    let (rx, rz) = rotate_8xz(roty, offx, offz);
    (rx, offy as i16, rz)
}

/// `strat_roffs_yaw` then uniform `ASL` post-scale.
#[inline]
pub fn strat_roffs_yaw_scaled(
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_yaw(roty, offx, offy, offz);
    (x << scale, y << scale, z << scale)
}

/// `strat_roffs_yaw` then per-axis `ASL` (ROM trailing X/Y/Z scale args may differ,
/// e.g. boss2plasma `2,0,4`).
#[inline]
pub fn strat_roffs_yaw_scaled_xyz(
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale_x: u32,
    scale_y: u32,
    scale_z: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_yaw(roty, offx, offy, offz);
    (x << scale_x, y << scale_y, z << scale_z)
}

/// i16 convenience for yaw-only Roffs (same scale-undo as `strat_roffs_full_i16`).
#[inline]
pub fn strat_roffs_yaw_i16(roty: u8, offx: i16, offy: i16, offz: i16) -> (i16, i16, i16) {
    let fits = |v: i16| (-128..=127).contains(&v);
    if fits(offx) && fits(offy) && fits(offz) {
        return strat_roffs_yaw(roty, offx as i8, offy as i8, offz as i8);
    }
    for scale in (1u32..=4).rev() {
        let sx = offx >> scale;
        let sy = offy >> scale;
        let sz = offz >> scale;
        if (sx << scale) == offx
            && (sy << scale) == offy
            && (sz << scale) == offz
            && fits(sx)
            && fits(sy)
            && fits(sz)
        {
            return strat_roffs_yaw_scaled(roty, sx as i8, sy as i8, sz as i8, scale);
        }
    }
    strat_roffs_yaw(roty, offx as i8, offy as i8, offz as i8)
}

/// ROM `mcoreZrot_srou` (GB3STRAT.ASM:4638): `rotz += vz >> 3` (signed).
#[inline]
pub fn mcore_zrot(rotz: &mut u8, vz: i16) {
    *rotz = rotz.wrapping_add((vz >> 3) as u8);
}

/// One output axis of `msh_rotpoints8` (MOBJ.MC:1678): sum three signed
/// products, double the total, select the high byte, then apply scale.
#[inline]
pub fn msh_packed8_axis(ma: i8, mb: i8, mc: i8, x: i8, y: i8, z: i8, scale: i8) -> i16 {
    let sum = (ma as i16)
        .wrapping_mul(x as i16)
        .wrapping_add((mb as i16).wrapping_mul(y as i16))
        .wrapping_add((mc as i16).wrapping_mul(z as i16));
    let scaled_high_byte = (((sum as i32) << 1) >> 8) as i8;
    (scaled_high_byte as i16).wrapping_mul(scale as i16)
}

/// ROM `msh_rotpoints8` packed matrix (when `m_shift < 3`).
/// `mat` bytes are Q7-ish object-matrix entries; `scale` is `m_scale`.
#[inline]
pub fn msh_rot_point8(mat: [[i8; 3]; 3], scale: i8, x: i8, y: i8, z: i8) -> (i16, i16, i16) {
    (
        msh_packed8_axis(mat[0][0], mat[1][0], mat[2][0], x, y, z, scale),
        msh_packed8_axis(mat[0][1], mat[1][1], mat[2][1], x, y, z, scale),
        msh_packed8_axis(mat[0][2], mat[1][2], mat[2][2], x, y, z, scale),
    )
}

/// Batch form of `msh_rot_point8`.
#[inline]
pub fn msh_rot_points8(
    mat: [[i8; 3]; 3],
    scale: i8,
    points: &[(i8, i8, i8)],
    out: &mut Vec<(i16, i16, i16)>,
) {
    out.clear();
    for &(x, y, z) in points.iter() {
        out.push(msh_rot_point8(mat, scale, x, y, z));
    }
}

/// ROM `msh_rotpoints8_16` (MOBJ.MC:1475): scale each byte by `m_scale`, then
/// rotate with the 16-bit FMULT path (`msh_rot_points16`).
#[inline]
pub fn msh_rot_points8_16(
    mat: [[i16; 3]; 3],
    scale: i8,
    points: &[(i8, i8, i8)],
    out: &mut Vec<(i16, i16, i16)>,
) {
    out.clear();
    let s = scale as i16;
    for &(x, y, z) in points {
        let sx = (x as i16).wrapping_mul(s);
        let sy = (y as i16).wrapping_mul(s);
        let sz = (z as i16).wrapping_mul(s);
        out.push(wmat_rot_point(mat, sx, sy, sz));
    }
}

/// ROM `mcalc_circle` midpoint loop (MCIRCLE.MC:67): Bresenham circle.
/// Fills `edges[y] = Some((x_left, x_right))` for scanlines that the circle
/// touches. Screen height matches the ROM buffer (224). The ROM plots the
/// +X octants into `m_circlebuf` and mirrors for the left edge in the clip
/// pass (`2*cx - x_right`).
pub fn mcalc_circle_edges(cx: i16, cy: i16, radius: i16, edges: &mut [Option<(i16, i16)>]) {
    edges.fill(None);
    if radius <= 0 {
        return;
    }
    let mut xp: i16 = 0;
    let mut yp = radius;
    let mut e: i16 = 1i16.wrapping_sub(yp);
    let mut u: i16 = 1;
    let mut v: i16 = e.wrapping_sub(yp); // 1 + 2x - 2y at x=0

    let plot = |edges: &mut [Option<(i16, i16)>], x: i16, y: i16| {
        if !(0..edges.len() as i16).contains(&y) {
            return;
        }
        let left = cx.wrapping_sub(x);
        let right = cx.wrapping_add(x);
        let slot = &mut edges[y as usize];
        *slot = Some(match *slot {
            None => (left, right),
            Some((l, r)) => (l.min(left), r.max(right)),
        });
    };

    let plot8 = |edges: &mut [Option<(i16, i16)>], xp: i16, yp: i16| {
        plot(edges, xp, cy.wrapping_add(yp));
        plot(edges, xp, cy.wrapping_sub(yp));
        plot(edges, yp, cy.wrapping_add(xp));
        plot(edges, yp, cy.wrapping_sub(xp));
    };

    // ROM: while xp < yp { plot; update }; then final plot at xp>=yp.
    while xp < yp {
        plot8(edges, xp, yp);
        xp = xp.wrapping_add(1);
        u = u.wrapping_add(2);
        v = v.wrapping_add(2);
        if e >= 0 {
            v = v.wrapping_add(2);
            e = e.wrapping_add(v);
            yp = yp.wrapping_sub(1);
        } else {
            e = e.wrapping_add(u);
        }
    }
    plot8(edges, xp, yp);
}
