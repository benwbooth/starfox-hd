#![allow(dead_code)]

//! Shared test helpers: source-asset title golden and region averaging.

/// Exact 8x8 averages from the source-asset CPU title compositor. The strict
/// external title-video oracle remains the authority for the dynamic screen;
/// this fixture protects deterministic tile decoding and priority composition.
#[rustfmt::skip]
pub const SOURCE_TITLE_COMPOSITE_GRID: [[u8; 3]; 64] = [
    [0,0,0],[9,6,4],[15,10,8],[3,1,2],[21,15,11],[10,6,5],[3,1,2],[0,0,0],
    [1,0,0],[77,54,36],[180,124,88],[191,134,90],[179,123,87],[183,127,88],[117,87,61],[0,0,0],
    [0,0,0],[24,16,14],[30,21,18],[79,50,39],[137,84,57],[104,66,47],[52,38,34],[1,0,0],
    [0,0,0],[0,0,0],[3,4,1],[13,23,4],[16,27,6],[2,4,0],[11,10,3],[0,0,0],
    [0,0,0],[1,0,0],[68,53,42],[47,41,30],[47,39,23],[6,6,5],[76,72,41],[1,1,0],
    [1,0,0],[0,0,0],[68,62,47],[119,97,58],[184,158,108],[64,60,53],[0,0,0],[0,0,0],
    [0,0,0],[3,3,3],[124,132,78],[73,68,44],[152,126,87],[112,105,76],[21,21,20],[0,0,0],
    [0,0,0],[2,2,3],[14,14,16],[34,33,32],[35,32,30],[35,32,31],[6,6,6],[0,0,0],
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
