//! Bit-exact ports of the ROM's fixed-point trig + multiply (STRATROU.ASM /
//! MACROS.INC). These replace float `sin/cos` * multiply, which drifted ~2%
//! per op vs the SNES hardware fixed-point and accumulated into visible wobble.
//! Verified against the real ROM by `sf-oracle` (tests/gen_3dvecs.rs).

/// ROM `sintab` table (STRATROU uses these exact bytes).
pub static SINTAB: [i8; 256] = [
    0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45,
    48, 51, 54, 57, 59, 62, 65, 67, 70, 73, 75, 78, 80, 82, 85, 87,
    89, 91, 94, 96, 98, 100, 102, 103, 105, 107, 108, 110, 112, 113, 114, 116,
    117, 118, 119, 120, 121, 122, 123, 123, 124, 125, 125, 126, 126, 126, 126, 126,
    127, 126, 126, 126, 126, 126, 125, 125, 124, 123, 123, 122, 121, 120, 119, 118,
    117, 116, 114, 113, 112, 110, 108, 107, 105, 103, 102, 100, 98, 96, 94, 91,
    89, 87, 85, 82, 80, 78, 75, 73, 70, 67, 65, 62, 59, 57, 54, 51,
    48, 45, 42, 39, 36, 33, 30, 27, 24, 21, 18, 15, 12, 9, 6, 3,
    0, -3, -6, -9, -12, -15, -18, -21, -24, -27, -30, -33, -36, -39, -42, -45,
    -48, -51, -54, -57, -59, -62, -65, -67, -70, -73, -75, -78, -80, -82, -85, -87,
    -89, -91, -94, -96, -98, -100, -102, -103, -105, -107, -108, -110, -112, -113, -114, -116,
    -117, -118, -119, -120, -121, -122, -123, -123, -124, -125, -125, -126, -126, -126, -126, -126,
    -127, -126, -126, -126, -126, -126, -125, -125, -124, -123, -123, -122, -121, -120, -119, -118,
    -117, -116, -114, -113, -112, -110, -108, -107, -105, -103, -102, -100, -98, -96, -94, -91,
    -89, -87, -85, -82, -80, -78, -75, -73, -70, -67, -65, -62, -59, -57, -54, -51,
    -48, -45, -42, -39, -36, -33, -30, -27, -24, -21, -18, -15, -12, -9, -6, -3,
];

/// ROM `costab` table (STRATROU uses these exact bytes).
pub static COSTAB: [i8; 256] = [
    127, 126, 126, 126, 126, 126, 125, 125, 124, 123, 123, 122, 121, 120, 119, 118,
    117, 116, 114, 113, 112, 110, 108, 107, 105, 103, 102, 100, 98, 96, 94, 91,
    89, 87, 85, 82, 80, 78, 75, 73, 70, 67, 65, 62, 59, 57, 54, 51,
    48, 45, 42, 39, 36, 33, 30, 27, 24, 21, 18, 15, 12, 9, 6, 3,
    0, -3, -6, -9, -12, -15, -18, -21, -24, -27, -30, -33, -36, -39, -42, -45,
    -48, -51, -54, -57, -59, -62, -65, -67, -70, -73, -75, -78, -80, -82, -85, -87,
    -89, -91, -94, -96, -98, -100, -102, -103, -105, -107, -108, -110, -112, -113, -114, -116,
    -117, -118, -119, -120, -121, -122, -123, -123, -124, -125, -125, -126, -126, -126, -126, -126,
    -127, -126, -126, -126, -126, -126, -125, -125, -124, -123, -123, -122, -121, -120, -119, -118,
    -117, -116, -114, -113, -112, -110, -108, -107, -105, -103, -102, -100, -98, -96, -94, -91,
    -89, -87, -85, -82, -80, -78, -75, -73, -70, -67, -65, -62, -59, -57, -54, -51,
    -48, -45, -42, -39, -36, -33, -30, -27, -24, -21, -18, -15, -12, -9, -6, -3,
    0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45,
    48, 51, 54, 57, 59, 62, 65, 67, 70, 73, 75, 78, 80, 82, 85, 87,
    89, 91, 94, 96, 98, 100, 102, 103, 105, 107, 108, 110, 112, 113, 114, 116,
    117, 118, 119, 120, 121, 122, 123, 123, 124, 125, 125, 126, 126, 126, 126, 126,
];

/// ROM `mulslog168` (MACROS.INC:2540) -> `muls816log16` (GAME.ASM:543) ->
/// `mla16mac m1,m3,m4,m6` with m6=0 (MACROS.INC:1303). Verified bit-exact vs the
/// real ROM by `sf-oracle` (tests/mulslog_oracle.rs).
///
/// The multiplicand `\1` is a SIGNED 16-bit word (m1/m2); the multiplier `\2` is
/// a SIGNED 8-bit byte (m3). Only those widths are read from the operands (`lda`
/// + `bmi` on each), so the i32 args are reinterpreted to match — a raw value
/// with bit 7 (b) or bit 15 (a) set is treated as negative exactly as the ROM
/// does. This is the "≥128 latent": a caller passing a table entry or velocity
/// byte with bit 7 set gets the ROM's signed-byte interpretation regardless of
/// whether the caller pre-sign-extended it.
#[inline]
pub fn mulslog(a: i32, b: i32) -> i32 {
    let x = (a as i16) as i32; // \1 = m1/m2: signed 16-bit word
    let f = (b as i8) as i32; //  \2 = m3:    signed 8-bit byte
    // fr = 2*|f|, computed in an 8-bit accumulator (`asl a`), so f == -128
    // overflows the byte to 0 — the ROM then multiplies by 0 (magnitude 0).
    let fr = ((f.unsigned_abs() << 1) & 0xFF) as i32;
    // magnitude = floor(|x| * fr / 256) = floor(|x| * |f| / 128), truncated,
    // matching the byte-wise accumulate `xh*fr + hi_byte(xl*fr)`.
    let mag = (x.unsigned_abs() as i32 * fr) >> 8;
    // Sign is the add/subtract path selected by sign(x) XOR sign(f).
    if (x < 0) ^ (f < 0) {
        -mag
    } else {
        mag
    }
}
