//! Shared ROM fixed-point trig (STRATROU / MACROS.INC): `sintab`/`costab`,
//! `mulslog` / `mulslogmac`, `rotate_8*` / `rotate_16*`, and the full/roll
//! `s_add_Roffs2pos` helpers.
//!
//! Used by `sf-game` / `sf-path` and re-exported by `sf-strat::snes_trig`
//! (which keeps the remaining `strat_roffs_*` flag variants).

/// ROM `sintab` (STRATROU).
pub static SINTAB: [i8; 256] = [
    0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 59, 62, 65, 67, 70,
    73, 75, 78, 80, 82, 85, 87, 89, 91, 94, 96, 98, 100, 102, 103, 105, 107, 108, 110, 112, 113,
    114, 116, 117, 118, 119, 120, 121, 122, 123, 123, 124, 125, 125, 126, 126, 126, 126, 126, 127,
    126, 126, 126, 126, 126, 125, 125, 124, 123, 123, 122, 121, 120, 119, 118, 117, 116, 114, 113,
    112, 110, 108, 107, 105, 103, 102, 100, 98, 96, 94, 91, 89, 87, 85, 82, 80, 78, 75, 73, 70, 67,
    65, 62, 59, 57, 54, 51, 48, 45, 42, 39, 36, 33, 30, 27, 24, 21, 18, 15, 12, 9, 6, 3, 0, -3, -6,
    -9, -12, -15, -18, -21, -24, -27, -30, -33, -36, -39, -42, -45, -48, -51, -54, -57, -59, -62,
    -65, -67, -70, -73, -75, -78, -80, -82, -85, -87, -89, -91, -94, -96, -98, -100, -102, -103,
    -105, -107, -108, -110, -112, -113, -114, -116, -117, -118, -119, -120, -121, -122, -123, -123,
    -124, -125, -125, -126, -126, -126, -126, -126, -127, -126, -126, -126, -126, -126, -125, -125,
    -124, -123, -123, -122, -121, -120, -119, -118, -117, -116, -114, -113, -112, -110, -108, -107,
    -105, -103, -102, -100, -98, -96, -94, -91, -89, -87, -85, -82, -80, -78, -75, -73, -70, -67,
    -65, -62, -59, -57, -54, -51, -48, -45, -42, -39, -36, -33, -30, -27, -24, -21, -18, -15, -12,
    -9, -6, -3,
];

/// ROM `costab` (STRATROU).
pub static COSTAB: [i8; 256] = [
    127, 126, 126, 126, 126, 126, 125, 125, 124, 123, 123, 122, 121, 120, 119, 118, 117, 116, 114,
    113, 112, 110, 108, 107, 105, 103, 102, 100, 98, 96, 94, 91, 89, 87, 85, 82, 80, 78, 75, 73,
    70, 67, 65, 62, 59, 57, 54, 51, 48, 45, 42, 39, 36, 33, 30, 27, 24, 21, 18, 15, 12, 9, 6, 3, 0,
    -3, -6, -9, -12, -15, -18, -21, -24, -27, -30, -33, -36, -39, -42, -45, -48, -51, -54, -57,
    -59, -62, -65, -67, -70, -73, -75, -78, -80, -82, -85, -87, -89, -91, -94, -96, -98, -100,
    -102, -103, -105, -107, -108, -110, -112, -113, -114, -116, -117, -118, -119, -120, -121, -122,
    -123, -123, -124, -125, -125, -126, -126, -126, -126, -126, -127, -126, -126, -126, -126, -126,
    -125, -125, -124, -123, -123, -122, -121, -120, -119, -118, -117, -116, -114, -113, -112, -110,
    -108, -107, -105, -103, -102, -100, -98, -96, -94, -91, -89, -87, -85, -82, -80, -78, -75, -73,
    -70, -67, -65, -62, -59, -57, -54, -51, -48, -45, -42, -39, -36, -33, -30, -27, -24, -21, -18,
    -15, -12, -9, -6, -3, 0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54,
    57, 59, 62, 65, 67, 70, 73, 75, 78, 80, 82, 85, 87, 89, 91, 94, 96, 98, 100, 102, 103, 105,
    107, 108, 110, 112, 113, 114, 116, 117, 118, 119, 120, 121, 122, 123, 123, 124, 125, 125, 126,
    126, 126, 126, 126,
];

/// First quadrant of retail `sintab16` (`$0000..$7FFF`, inclusive).
/// The other quadrants are exact reflections of these 65 samples.
const SINTAB16_QUARTER: [i16; 65] = [
    0x0000, 0x0324, 0x0647, 0x096A, 0x0C8B, 0x0FAB, 0x12C7, 0x15E1, 0x18F8, 0x1C0B, 0x1F19, 0x2223,
    0x2527, 0x2826, 0x2B1E, 0x2E10, 0x30FB, 0x33DE, 0x36B9, 0x398C, 0x3C56, 0x3F16, 0x41CD, 0x447A,
    0x471C, 0x49B3, 0x4C3F, 0x4EBF, 0x5133, 0x539A, 0x55F4, 0x5842, 0x5A81, 0x5CB3, 0x5ED6, 0x60EB,
    0x62F1, 0x64E7, 0x66CE, 0x68A5, 0x6A6C, 0x6C23, 0x6DC9, 0x6F5E, 0x70E1, 0x7254, 0x73B5, 0x7503,
    0x7640, 0x776B, 0x7883, 0x7989, 0x7A7C, 0x7B5C, 0x7C29, 0x7CE2, 0x7D89, 0x7E1C, 0x7E9C, 0x7F08,
    0x7F61, 0x7FA6, 0x7FD7, 0x7FF5, 0x7FFF,
];

/// One exact Q15 sample from retail `sintab16` for a byte angle.
#[inline]
pub fn sin_q15(angle: u8) -> i16 {
    match angle {
        0x00..=0x40 => SINTAB16_QUARTER[angle as usize],
        0x41..=0x7F => SINTAB16_QUARTER[(0x80 - angle as u16) as usize],
        0x80..=0xC0 => -SINTAB16_QUARTER[(angle as u16 - 0x80) as usize],
        _ => -SINTAB16_QUARTER[(0x100 - angle as u16) as usize],
    }
}

#[inline]
pub fn cos_q15(angle: u8) -> i16 {
    sin_q15(angle.wrapping_add(0x40))
}

/// One interpolated Q15 sine sample for the original 16-bit angle format.
/// The high byte selects adjacent `sintab16` entries and the low byte is the
/// linear interpolation fraction used by the GSU `mgetsin16` macro.
#[inline]
pub fn sin_q15_fine(angle: u16) -> i16 {
    const INTERPOLATION_SHIFT: u32 = 7;
    let whole = (angle >> 8) as u8;
    let fraction = (angle & 255) << INTERPOLATION_SHIFT;
    let first = sin_q15(whole);
    let second = sin_q15(whole.wrapping_add(1));
    let delta = second.wrapping_sub(first);
    first.wrapping_add(gsu_fmult_q15(delta, fraction as i16))
}

#[inline]
pub fn cos_q15_fine(angle: u16) -> i16 {
    sin_q15_fine(angle.wrapping_add(16_384))
}

/// GSU `FMULT; ROL`: signed `(a*b)>>15`, truncated to 16 bits.
#[inline]
pub fn gsu_fmult_q15(a: i16, b: i16) -> i16 {
    (((a as i32) * (b as i32)) >> 15) as i16
}

/// Retail `mcrotmatzxy16` for byte-aligned 16-bit angles. The returned matrix
/// is row-major and preserves the GSU's per-product truncation and 16-bit
/// wrapping additions.
pub fn zxy_matrix_q15(rx: u8, ry: u8, rz: u8) -> [[i16; 3]; 3] {
    let sx = sin_q15(rx);
    let cx = cos_q15(rx);
    let sy = sin_q15(ry);
    let cy = cos_q15(ry);
    let sz = sin_q15(rz);
    let cz = cos_q15(rz);

    let t1 = gsu_fmult_q15(cz, sy);
    let t2 = gsu_fmult_q15(cz, cy);
    let t3 = gsu_fmult_q15(sz, sy);
    let t4 = gsu_fmult_q15(sz, cy);
    [
        [
            gsu_fmult_q15(t3, sx).wrapping_add(t2),
            gsu_fmult_q15(t1, sx).wrapping_sub(t4),
            gsu_fmult_q15(cx, sy),
        ],
        [
            gsu_fmult_q15(cx, sz),
            gsu_fmult_q15(cx, cz),
            sx.wrapping_neg(),
        ],
        [
            gsu_fmult_q15(t4, sx).wrapping_sub(t1),
            gsu_fmult_q15(t2, sx).wrapping_add(t3),
            gsu_fmult_q15(cx, cy),
        ],
    ]
}

/// Retail `mcrotmatzxy16`, including the low-byte interpolation of each
/// authored 16-bit view angle.
pub fn zxy_matrix_q15_fine(pitch: u16, yaw: u16, roll: u16) -> [[i16; 3]; 3] {
    let sx = sin_q15_fine(pitch);
    let cx = cos_q15_fine(pitch);
    let sy = sin_q15_fine(yaw);
    let cy = cos_q15_fine(yaw);
    let sz = sin_q15_fine(roll);
    let cz = cos_q15_fine(roll);

    let t1 = gsu_fmult_q15(cz, sy);
    let t2 = gsu_fmult_q15(cz, cy);
    let t3 = gsu_fmult_q15(sz, sy);
    let t4 = gsu_fmult_q15(sz, cy);
    [
        [
            gsu_fmult_q15(t3, sx).wrapping_add(t2),
            gsu_fmult_q15(t1, sx).wrapping_sub(t4),
            gsu_fmult_q15(cx, sy),
        ],
        [
            gsu_fmult_q15(cx, sz),
            gsu_fmult_q15(cx, cz),
            sx.wrapping_neg(),
        ],
        [
            gsu_fmult_q15(t4, sx).wrapping_sub(t1),
            gsu_fmult_q15(t2, sx).wrapping_add(t3),
            gsu_fmult_q15(cx, cy),
        ],
    ]
}

/// Retail `mwmatrotp16`: rotate one point with per-term Q15 truncation.
pub fn matrix_rotate_q15(matrix: [[i16; 3]; 3], x: i16, y: i16, z: i16) -> (i16, i16, i16) {
    let dot = |c0: i16, c1: i16, c2: i16| {
        gsu_fmult_q15(x, c0)
            .wrapping_add(gsu_fmult_q15(y, c1))
            .wrapping_add(gsu_fmult_q15(z, c2))
    };
    (
        dot(matrix[0][0], matrix[1][0], matrix[2][0]),
        dot(matrix[0][1], matrix[1][1], matrix[2][1]),
        dot(matrix[0][2], matrix[1][2], matrix[2][2]),
    )
}

/// One output axis of the source renderer's packed-point transform.
///
/// The three signed products accumulate in a wrapping word. The source then
/// doubles that word, keeps its high byte as a signed value, and applies the
/// shape's authored power-of-two scale.
#[inline]
pub fn packed_point_axis(
    coefficient_a: i8,
    coefficient_b: i8,
    coefficient_c: i8,
    x: i8,
    y: i8,
    z: i8,
    scale: i8,
) -> i16 {
    let sum = i16::from(coefficient_a)
        .wrapping_mul(i16::from(x))
        .wrapping_add(i16::from(coefficient_b).wrapping_mul(i16::from(y)))
        .wrapping_add(i16::from(coefficient_c).wrapping_mul(i16::from(z)));
    let high_byte = (((i32::from(sum)) << 1) >> 8) as i8;
    i16::from(high_byte).wrapping_mul(i16::from(scale))
}

/// Transform one signed-byte point with the packed source object matrix.
#[inline]
pub fn rotate_packed_point(
    matrix: [[i8; 3]; 3],
    scale: i8,
    x: i8,
    y: i8,
    z: i8,
) -> (i16, i16, i16) {
    (
        packed_point_axis(
            matrix[0][0],
            matrix[1][0],
            matrix[2][0],
            x,
            y,
            z,
            scale,
        ),
        packed_point_axis(
            matrix[0][1],
            matrix[1][1],
            matrix[2][1],
            x,
            y,
            z,
            scale,
        ),
        packed_point_axis(
            matrix[0][2],
            matrix[1][2],
            matrix[2][2],
            x,
            y,
            z,
            scale,
        ),
    )
}

/// ROM `mulslogmac` (MACROS.INC:911) — signed 8×8 → signed 8.
#[inline]
pub fn mulslog_mac8(a: i8, b: i8) -> i8 {
    let au = (a as i32).wrapping_abs() as u8;
    let bu = (b as i32).wrapping_abs() as u8;
    let fr = au.wrapping_shl(1);
    let mag = ((fr as u16 * bu as u16) >> 8) as u8;
    if (a < 0) ^ (b < 0) {
        (0i16 - mag as i16) as i8
    } else {
        mag as i8
    }
}

/// ROM `rotate_8yx_l` (STRATROU.ASM:1128). Angle is NOT negated.
#[inline]
pub fn rotate_8yx(angle: u8, x: i8, y: i8) -> (i16, i16) {
    let cos = COSTAB[angle as usize];
    let sin = SINTAB[angle as usize];
    let x2 = mulslog_mac8(x, cos).wrapping_add(mulslog_mac8(y, sin));
    let y2 = (-mulslog_mac8(x, sin)).wrapping_add(mulslog_mac8(y, cos));
    (x2 as i16, y2 as i16)
}

/// ROM `mulslog168` / `muls816log16` — signed 16×8 → 16 magnitude (MACROS.INC).
#[inline]
pub fn mulslog(a: i32, b: i32) -> i32 {
    let x = (a as i16) as i32;
    let f = (b as i8) as i32;
    let fr = ((f.unsigned_abs() << 1) & 0xFF) as i32;
    let mag = (x.unsigned_abs() as i32 * fr) >> 8;
    if (x < 0) ^ (f < 0) {
        -mag
    } else {
        mag
    }
}

/// ROM `rotate_16xz_l` (STRATROU.ASM:1198). Angle is NOT auto-negated
/// (callers that need `nega` must pass it). Returns `(x', z')`.
#[inline]
pub fn rotate_16xz(angle: u8, x: i16, z: i16) -> (i16, i16) {
    let sin = SINTAB[angle as usize] as i32;
    let cos = COSTAB[angle as usize] as i32;
    let xi = x as i32;
    let zi = z as i32;
    let z_out = mulslog(xi, sin).wrapping_add(mulslog(zi, cos));
    let x_out = mulslog(xi, cos).wrapping_sub(mulslog(zi, sin));
    (x_out as i16, z_out as i16)
}

/// ROM `rotate_16yz_l` (STRATROU.ASM:1276). Angle is negated before lookup.
#[inline]
pub fn rotate_16yz(angle: u8, y: i16, z: i16) -> (i16, i16) {
    let neg = angle.wrapping_neg();
    let sin = SINTAB[neg as usize] as i32;
    let cos = COSTAB[neg as usize] as i32;
    let yi = y as i32;
    let zi = z as i32;
    let z_out = mulslog(yi, sin).wrapping_add(mulslog(zi, cos));
    let y_out = mulslog(yi, cos).wrapping_sub(mulslog(zi, sin));
    (y_out as i16, z_out as i16)
}

/// ROM `rotate_8xz_l` (STRATROU.ASM:986). Angle is negated before lookup.
#[inline]
pub fn rotate_8xz(angle: u8, x: i8, z: i8) -> (i16, i16) {
    let neg = angle.wrapping_neg();
    let cos = COSTAB[neg as usize];
    let sin = SINTAB[neg as usize];
    let x2 = mulslog_mac8(x, cos).wrapping_add(mulslog_mac8(z, sin));
    let z2 = (-mulslog_mac8(x, sin)).wrapping_add(mulslog_mac8(z, cos));
    (x2 as i16, z2 as i16)
}

/// ROM `rotate_8yz_l` (STRATROU.ASM:1057). Angle is NOT negated.
#[inline]
pub fn rotate_8yz(angle: u8, y: i8, z: i8) -> (i16, i16) {
    let cos = COSTAB[angle as usize];
    let sin = SINTAB[angle as usize];
    let y2 = mulslog_mac8(y, cos).wrapping_add(mulslog_mac8(z, sin));
    let z2 = (-mulslog_mac8(y, sin)).wrapping_add(mulslog_mac8(z, cos));
    (y2 as i16, z2 as i16)
}

/// ROM `s_add_Roffs2pos` B-mode flags 1,1,1 — `rotate_8yx` → `rotate_8yz` →
/// `rotate_8xz`. Between stages only the low byte is fed forward.
#[inline]
pub fn strat_roffs_full(
    rotz: u8,
    rotx: u8,
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
) -> (i16, i16, i16) {
    let (x1, y1) = rotate_8yx(rotz, offx, offy);
    let (y2, z2) = rotate_8yz(rotx, y1 as i8, offz);
    let (x3, z3) = rotate_8xz(roty, x1 as i8, z2 as i8);
    (x3, y2, z3)
}

/// `strat_roffs_full` then `ASL` each axis `scale` times.
#[inline]
pub fn strat_roffs_full_scaled(
    rotz: u8,
    rotx: u8,
    roty: u8,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) -> (i16, i16, i16) {
    let (x, y, z) = strat_roffs_full(rotz, rotx, roty, offx, offy, offz);
    (x << scale, y << scale, z << scale)
}

/// ROM `s_add_Roffs2pos` B-mode flags 0,0,1 — Z/roll only via `rotate_8yx_l`.
#[inline]
pub fn strat_roffs_roll(rotz: u8, offx: i8, offy: i8, offz: i8) -> (i16, i16, i16) {
    let (x2, y2) = rotate_8yx(rotz, offx, offy);
    (x2, y2, offz as i16)
}

/// ROM B-mode `#Xspacebarlen/2` (500/2=250) after `sexam` → i8 −6.
pub const XSPACEBAR_HALF_B: i8 = (500i16 / 2) as i8;

/// ROM `Achase_var2A` / `sr8_achase_alvarN` — 8-bit proportional chase.
/// `diff = target - current`, toward-zero `adiv2` × shift, min |step|=1.
/// Returns true only when already at target on entry.
#[inline]
pub fn achase_angle_8(current: &mut u8, target: u8, shift: u32) -> bool {
    if *current == target {
        return true;
    }
    let diff = (target.wrapping_sub(*current) as i8) as i32;
    let mut step = if diff >= 0 {
        diff >> shift
    } else {
        -((-diff) >> shift)
    };
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    *current = current.wrapping_add(step as u8);
    false
}
