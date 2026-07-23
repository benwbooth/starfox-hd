//! View-matrix regression guard: a look-at target placed along the camera's
//! exact look direction must render near screen center for pure yaw, pure
//! pitch, AND composed yaw+pitch (the Corneria opening look-back camera).
//! Guards the V = Rz(-rz)*Rx(-rx)*Ry(-ry) world->camera construction + the
//! SNES->GL basis scalings in transform.rs — a transpose/sign hack that
//! passes the single-axis cases but breaks composition turned the whole
//! opening cinematic into a solid green screen.
use sf_render::draw_list::{DrawListEntry, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};
use sf_render::shapes::SHAPE_MYSHIP_4;
use std::path::PathBuf;

#[test]
fn opening_lookback_camera_renders_ship() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut r = match Renderer::new_headless(1280, 720, &config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: {e}");
            return;
        }
    };
    // Bisect: which camera rotation component hides the target?
    // Each case: camera + a ship placed exactly along the camera's look
    // direction at ~1400 units (should be screen-center every time).
    let cases: &[(&str, i32, i32, i32, i16, i16, i16, i32, i32, i32)] = &[
        ("identity      ", 0, 0, 0, 0, 0, 0, 0, 0, 1400),
        ("yaw 32 (45d)  ", 0, 0, 0, 0, 32, 0, 990, 0, 990),
        ("yaw 64 (90d)  ", 0, 0, 0, 0, 64, 0, 1400, 0, 0),
        ("yaw 108       ", 0, 0, 0, 0, 108, 0, 660, 0, -1234),
        ("pitch -27     ", 0, 0, 0, -27, 0, 0, 0, 870, 1096),
        (
            "opening comb  ",
            -831,
            -1411,
            2832,
            -27,
            108,
            0,
            0,
            -101,
            1000,
        ),
    ];
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        ..Default::default()
    };
    for &(name, cx, cy, cz, rx, ry, rz, sx, sy, sz) in cases {
        r.transform
            .set_camera(cx << 16, cy << 16, cz << 16, rx, ry, rz);
        let e = DrawListEntry {
            shape_id: SHAPE_MYSHIP_4,
            flags: DL_FLAG_VISIBLE,
            x: sx << 16,
            y: sy << 16,
            z: sz << 16,
            ry: 128,
            obj_id: 1,
            ..Default::default()
        };
        r.begin_frame();
        r.submit(&[e], &[e], 1.0, &inputs);
        r.end_frame();
        let px = r.read_pixels_rgb();
        let mut non_bg = 0usize;
        let (mut cxs, mut cys) = (0i64, 0i64);
        for (i, p) in px.chunks_exact(3).enumerate() {
            if p[0] as i32 + p[1] as i32 + p[2] as i32 > 45 {
                non_bg += 1;
                cxs += (i % 1280) as i64;
                cys += (i / 1280) as i64;
            }
        }
        assert!(non_bg > 40, "{name}: ship not rendered (non_bg={non_bg})");
        let (cx_px, cy_px) = (cxs / non_bg as i64, cys / non_bg as i64);
        eprintln!("PROBE {name} non_bg={non_bg} centroid=({cx_px},{cy_px})");
        // Single-axis cases aim exactly at the ship -> tight center bound;
        // the combined case uses the game-trace approximate angles -> loose.
        let tol: i64 = if name.trim() == "opening comb" {
            220
        } else {
            60
        };
        assert!(
            (cx_px - 640).abs() < tol && (cy_px - 360).abs() < tol,
            "{name}: ship off-center at ({cx_px},{cy_px})"
        );
    }
    r.shutdown();
}
