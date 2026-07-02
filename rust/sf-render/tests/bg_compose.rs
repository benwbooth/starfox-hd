//! CPU-side background composer parity tests (no GL context needed).
//!
//! Verifies the bg2d composers against known values from the C oracle:
//!  - bg_1_1c (ST-P.CGX + BG2-D.COL): the screen-top row at the base scroll
//!    (bg2Yscroll 232) is the uniform Corneria sky blue RGB(49, 90, 148).
//!  - The composed title screen matches an 8x8 region-average golden grid
//!    captured ONCE from the C build (SF_DUMP_PPM on the stable title
//!    frame, 2026-07-01). Per-region tolerance covers GPU-scaling noise in
//!    the capture; the compose itself is exact integer math.

use std::path::PathBuf;

use sf_render::bg2d::{compose_bg, compose_title, BG2D_H, BG2D_W};

mod common;
use common::{grid_8x8, C_TITLE_GOLDEN_8X8};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> Vec<u8> {
    let p = repo_root().join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("missing asset {}: {e}", p.display()))
}

#[test]
fn bg_1_1c_sky_top_row_is_corneria_blue() {
    let cgx = read("data/bg/ST-P.CGX");
    let scr = read("data/bg/ST-P.SCR");
    let col = read("data/bg/BG2-D.COL");

    // bg_1_1c def: vofs 232, sky-coupled -> full wrapping 512x512 tilemap.
    let (rgba, w, h) = compose_bg(&cgx, &scr, &col, None, None, 232, 0, true)
        .expect("compose_bg failed");
    assert_eq!((w, h), (512, 512));

    // At the base scroll the screen-top row shows map row 232. The compose
    // output is bottom-up (GL row 0 = map bottom), so map row m lives at
    // output row (h - 1 - m).
    let out_row = h - 1 - 232;
    for x in 0..w {
        let px = &rgba[(out_row * w + x) * 4..][..4];
        assert_eq!(
            (px[0], px[1], px[2], px[3]),
            (49, 90, 148, 255),
            "bg_1_1c map row 232, column {x}: expected uniform sky blue"
        );
    }
}

#[test]
fn title_compose_matches_c_golden_grid() {
    let ti_cgx = read("data/title/TI-3-US.CGX");
    let ti_scr = read("data/title/TI-3-US.SCR");
    let cp_cgx = read("data/title/CP.CGX");
    let cp_scr = read("data/title/CP.SCR");
    let col = read("data/title/CP-US.COL");

    let rgba = compose_title(&ti_cgx, &ti_scr, &cp_cgx, &cp_scr, &col)
        .expect("compose_title failed");
    assert_eq!(rgba.len(), BG2D_W * BG2D_H * 4);

    // The compose output is bottom-up; flip to top-down for the grid.
    let mut top_down = vec![0u8; rgba.len()];
    for y in 0..BG2D_H {
        top_down[y * BG2D_W * 4..(y + 1) * BG2D_W * 4]
            .copy_from_slice(&rgba[(BG2D_H - 1 - y) * BG2D_W * 4..(BG2D_H - y) * BG2D_W * 4]);
    }

    let grid = grid_8x8(&top_down, BG2D_W, BG2D_H, 4);
    for (i, (got, want)) in grid.iter().zip(C_TITLE_GOLDEN_8X8.iter()).enumerate() {
        for c in 0..3 {
            let delta = (got[c] as i32 - want[c] as i32).abs();
            assert!(
                delta <= 3,
                "title region {i} channel {c}: got {} want {} (delta {delta})",
                got[c],
                want[c]
            );
        }
    }
}
