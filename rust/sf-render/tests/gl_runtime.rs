//! Offscreen wgpu runtime tests: a headless wgpu device rendering into a
//! texture (no window/display needed), full Renderer pass pipeline.
//!
//! One #[test] runs all checks sequentially against a single headless
//! renderer.
//!
//! Checks:
//!  (a) Arwing (SHAPE_MYSHIP_4) rendered with a known camera: readback
//!      contains the canopy blue family (NIGHT.COL CA_2 cycle -> palette 8
//!      at col_frame 0) and a COLLITE hull grey shade.
//!  (b) bg_1_1c playing-state frame: screen-top row is the composed sky
//!      blue RGB(49, 90, 148) after the calcbgscroll_l coupling at rx=0.
//!  (c) Title frame vs the C-build golden (8x8 region averages captured
//!      once from SF_DUMP_PPM; exact hash impractical across GPU scaling,
//!      so per-region average deltas must stay <= 4).

use std::path::PathBuf;

use sf_render::draw_list::{DrawListEntry, DL_FLAG_VISIBLE};
use sf_render::renderer::{
    config_from_repo_root, FrameInputs, GameState, Renderer,
};
use sf_render::shapes::{self, SHAPE_MYSHIP_4};

mod common;
use common::{grid_8x8, C_TITLE_GOLDEN_8X8};

const W: u32 = 1280;
const H: u32 = 720;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn color_near(px: &[u8], want: [u8; 3], tol: i32) -> bool {
    (0..3).all(|c| (px[c] as i32 - want[c] as i32).abs() <= tol)
}

fn expected_rgb8(color: [f32; 4]) -> [u8; 3] {
    [
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
    ]
}

#[test]
fn gl_runtime_suite() {
    let config = config_from_repo_root(&repo_root());
    let mut renderer = match Renderer::new_headless(W as i32, H as i32, &config) {
        Ok(r) => r,
        // No usable GPU adapter in this environment (e.g. CI without a
        // software rasterizer) — skip rather than fail.
        Err(e) => {
            eprintln!("skipping gl_runtime_suite: no wgpu adapter ({e})");
            return;
        }
    };

    check_title_golden(&mut renderer);
    check_bg_1_1c_sky(&mut renderer);
    check_arwing(&mut renderer);

    renderer.shutdown();
}

// (c) Full composed title frame vs C-build golden region averages.
fn check_title_golden(renderer: &mut Renderer) {
    let inputs = FrameInputs {
        game_state: GameState::Title,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 0.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    let grid = grid_8x8(&px, W as usize, H as usize, 3);
    let mut max_delta = 0i32;
    for (i, (got, want)) in grid.iter().zip(C_TITLE_GOLDEN_8X8.iter()).enumerate() {
        for c in 0..3 {
            let delta = (got[c] as i32 - want[c] as i32).abs();
            max_delta = max_delta.max(delta);
            assert!(
                delta <= 4,
                "title GL region {i} channel {c}: got {} want {} (delta {delta})",
                got[c],
                want[c]
            );
        }
    }
    println!("title golden: max region delta {max_delta}");
}

// (b) bg_1_1c: playing state on map 1_1 with a level camera at rx=0. The
// base bg2Yscroll (232) plus the SNES-vs-port horizon offset (+18, so the
// painted horizon locks to the port's y=0 vanishing line — see
// bg2d::sky_uv_window) windows a uniform sky-blue row at the screen top.
fn check_bg_1_1c_sky(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let inputs = FrameInputs {
        game_state: GameState::Playing,
        newmap: 1, // MAP_ID_1_1 -> default bg 4 (bg_1_1c)
        currentbg: 0,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();
    // Sample the top row (a few columns across the screen).
    for x in [10usize, W as usize / 2, W as usize - 10] {
        let p = &px[x * 3..x * 3 + 3];
        assert!(
            color_near(p, [49, 98, 156], 2),
            "bg_1_1c top row at x={x}: got ({}, {}, {}), want sky blue (49, 98, 156)",
            p[0],
            p[1],
            p[2]
        );
    }
}

// (a) Arwing with a known camera: canopy blue + hull grey present.
fn check_arwing(renderer: &mut Renderer) {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);

    // Two orientations so both canopy side-faces are unambiguously visible:
    // one seen from the front-top, one from the rear-top.
    let base = DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        flags: DL_FLAG_VISIBLE,
        ..Default::default()
    };
    let curr = [
        DrawListEntry {
            x: -70 << 16,
            y: 0,
            z: 150 << 16,
            rx: 32, // pitch toward the camera
            ry: 128,
            obj_id: 1,
            ..base
        },
        DrawListEntry {
            x: 70 << 16,
            y: 0,
            z: 150 << 16,
            rx: 224,
            ry: 0,
            obj_id: 2,
            ..base
        },
    ];

    let inputs = FrameInputs {
        game_state: GameState::Boot, // no bg/hud/ui passes; clear color only
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&curr, &curr, 1.0, &inputs);
    renderer.end_frame();

    let px = renderer.read_pixels_rgb();

    // Expected canopy blue: face color 44 (COLANIM CA_2), col_frame 0 ->
    // COLNORM palette 8 = NIGHT.COL 0x7FB6. Depth bank 0 (150 < 2560).
    let canopy = expected_rgb8(shapes::resolve_face_color(SHAPE_MYSHIP_4, 44, 0, 0, 9, 0));
    // Sanity-pin the family: NIGHT.COL palettes 5-8 decode to the blue ramp;
    // palette 8 (0x7FB6) is (181, 239, 255) in 8-bit.
    assert_eq!(canopy, [181, 239, 255], "canopy material decode changed");

    // Expected hull greys: COLLITE rows 0 and 1 across all shade indices.
    let mut hull_greys: Vec<[u8; 3]> = Vec::new();
    for face_color in [0u8, 1u8] {
        for shade in 0..10 {
            hull_greys.push(expected_rgb8(shapes::resolve_face_color(
                SHAPE_MYSHIP_4,
                face_color,
                0,
                0,
                shade,
                0,
            )));
        }
    }

    let mut non_bg = 0usize;
    let mut canopy_hits = 0usize;
    let mut hull_hits = 0usize;
    for p in px.chunks_exact(3) {
        if !color_near(p, [0, 0, 13], 2) {
            non_bg += 1;
        }
        if color_near(p, canopy, 3) {
            canopy_hits += 1;
        }
        if hull_greys.iter().any(|g| color_near(p, *g, 3)) {
            hull_hits += 1;
        }
    }

    println!("arwing: non_bg={non_bg} canopy_hits={canopy_hits} hull_hits={hull_hits}");
    assert!(non_bg > 500, "arwing barely rendered: {non_bg} non-bg pixels");
    assert!(canopy_hits > 10, "canopy blue not found ({canopy_hits} hits)");
    assert!(hull_hits > 50, "hull greys not found ({hull_hits} hits)");
}
