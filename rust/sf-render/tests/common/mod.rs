//! Shared test helpers: the C-build title golden and region averaging.

/// 8x8 grid of per-region average colors captured ONCE from the C build's
/// stable title frame (build/starfox-hd, SF_DUMP_PPM + SF_DUMP_SEQ,
/// 2026-07-01; frames stable to delta 0 across dumps). Top-down rows.
/// A Python re-implementation of the composer reproduced this grid within
/// +-2 per channel, so a tolerance of 3-4 covers GPU-scaling noise.
#[rustfmt::skip]
pub const C_TITLE_GOLDEN_8X8: [[u8; 3]; 64] = [
    [0,0,0],[13,10,7],[11,7,6],[3,1,2],[25,18,13],[5,3,3],[2,1,1],[0,0,0],
    [1,1,0],[96,66,45],[182,126,88],[192,135,90],[180,124,86],[183,127,88],[92,68,50],[0,0,0],
    [0,0,0],[29,19,16],[30,21,18],[92,58,44],[138,84,57],[90,58,42],[49,36,33],[1,0,0],
    [0,0,0],[0,0,0],[3,4,1],[13,22,4],[16,26,6],[2,4,0],[11,10,3],[0,0,0],
    [0,0,0],[1,0,0],[66,52,41],[46,40,30],[45,37,22],[6,6,5],[76,72,41],[1,1,0],
    [1,1,0],[0,0,0],[66,61,46],[118,96,57],[184,157,107],[64,59,53],[0,0,0],[0,0,0],
    [0,0,0],[3,3,3],[123,131,77],[73,68,44],[152,127,87],[112,104,76],[21,21,20],[0,0,0],
    [0,0,0],[2,2,2],[15,15,16],[33,33,32],[39,36,34],[31,28,28],[6,6,6],[0,0,0],
];

/// Compute 8x8 region average colors of a top-down RGB(A) image.
pub fn grid_8x8(px: &[u8], w: usize, h: usize, stride: usize) -> [[u8; 3]; 64] {
    let mut out = [[0u8; 3]; 64];
    for gy in 0..8 {
        for gx in 0..8 {
            let (y0, y1) = (gy * h / 8, (gy + 1) * h / 8);
            let (x0, x1) = (gx * w / 8, (gx + 1) * w / 8);
            let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * stride;
                    r += px[i] as u64;
                    g += px[i + 1] as u64;
                    b += px[i + 2] as u64;
                    n += 1;
                }
            }
            out[gy * 8 + gx] = [(r / n) as u8, (g / n) as u8, (b / n) as u8];
        }
    }
    out
}
