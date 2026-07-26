//! Smoke test: run the built binary headless-ish (hidden window) under
//! SF_AUTOPLAY for 600 ticks (~30 s of game time), dump the state trace and
//! one frame readback, and assert the ENDSEQ/MAIN state-machine transitions:
//! BOOT -> ATTRACT_INTRO -> TITLE -> BRIEFING -> TRAINING -> BRIEFING ->
//! PLANET_SELECT -> PLAYING.
//!
//! Skips (with a message) when DISPLAY is not set. The C-oracle comparison
//! (same env against build/starfox-hd) is a separate manual/differential
//! step — object-level parity is not asserted here because the strategy
//! lane has not landed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static SMOKE_LOCK: Mutex<()> = Mutex::new(());

const STATE_BOOT: u8 = 0;
const STATE_TITLE: u8 = 1;
const STATE_BRIEFING: u8 = 2;
const STATE_PLANET_SELECT: u8 = 3;
const STATE_PLAYING: u8 = 4;
const STATE_ATTRACT_INTRO: u8 = 8;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch_dir() -> PathBuf {
    let d = std::env::var("SF_SMOKE_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("sf_app_smoke"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Parse the "T <tick> <pad> <state> <ndraw>" lines into (state, ndraw).
fn parse_states(dump: &str) -> Vec<(u8, u32)> {
    dump.lines()
        .filter(|l| l.starts_with("T "))
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            Some((f.get(3)?.parse().ok()?, f.get(4)?.parse().ok()?))
        })
        .collect()
}

fn dedup_states(states: &[(u8, u32)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for (s, _) in states {
        if out.last() != Some(s) {
            out.push(*s);
        }
    }
    out
}

fn ppm_color_stats(path: &Path) -> (usize, usize) {
    let (_, _, pixels) = ppm_rgb(path);
    let colors: std::collections::BTreeSet<_> = pixels
        .chunks_exact(3)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let visible_pixels = pixels
        .chunks_exact(3)
        .filter(|pixel| **pixel != [0, 0, 0])
        .count();
    (colors.len(), visible_pixels)
}

fn ppm_rgb(path: &Path) -> (usize, usize, Vec<u8>) {
    let ppm = std::fs::read(path).expect("PPM readback missing");
    assert!(ppm.starts_with(b"P6\n"), "frame is not a P6 PPM");
    let body_start = ppm
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'\n')
        .nth(2)
        .map(|(index, _)| index + 1)
        .expect("PPM header is incomplete");
    let header = std::str::from_utf8(&ppm[..body_start]).expect("PPM header is not UTF-8");
    let mut fields = header.split_ascii_whitespace();
    assert_eq!(fields.next(), Some("P6"));
    let width = fields
        .next()
        .and_then(|value| value.parse().ok())
        .expect("PPM width is missing");
    let height = fields
        .next()
        .and_then(|value| value.parse().ok())
        .expect("PPM height is missing");
    assert_eq!(fields.next(), Some("255"));
    let pixels = ppm[body_start..].to_vec();
    assert_eq!(pixels.len(), width * height * 3, "PPM body size mismatch");
    (width, height, pixels)
}

#[test]
fn autoplay_reaches_gameplay() {
    let _serial = SMOKE_LOCK.lock().unwrap();
    if std::env::var("DISPLAY").unwrap_or_default().is_empty() {
        eprintln!("smoke: DISPLAY not set — skipping GL smoke test");
        return;
    }
    let root = repo_root();
    // Assets live in build/ (the C build tree); fall back to the repo root
    // if a data/ tree exists there.
    let cwd = if root.join("build/data").is_dir() {
        root.join("build")
    } else {
        root.clone()
    };
    assert!(
        cwd.join("data").is_dir(),
        "no data/ asset tree under {}",
        cwd.display()
    );

    let scratch = scratch_dir();
    let dump_path = scratch.join("app_rs_state.txt");
    let ppm_path = scratch.join("app_rs_frame.ppm");

    let bin = env!("CARGO_BIN_EXE_starfox-hd-rs");
    let status = Command::new(bin)
        .current_dir(&cwd)
        .env("SF_AUTOPLAY", "1")
        .env("SF_HIDDEN", "1")
        .env("SF_MAX_TICKS", "600") // 30 s of game time at 20 Hz
        .env("SF_STATE_DUMP", &dump_path)
        .env("SF_DUMP_PPM", &ppm_path)
        .status()
        .expect("failed to launch starfox-hd-rs");
    assert!(status.success(), "binary exited with {status:?}");

    let dump = std::fs::read_to_string(&dump_path).expect("state dump missing");
    let states = parse_states(&dump);
    assert_eq!(states.len(), 600, "expected 600 dumped ticks");

    let seq = dedup_states(&states);
    eprintln!("smoke: state sequence {seq:?}");
    // The first dumped tick already shows ATTRACT_INTRO because Game_Init
    // runs inside tick zero. Preserve tolerance for an explicitly dumped
    // BOOT state while requiring the complete retail attract handoff.
    let expected_tail: &[u8] = &[
        STATE_ATTRACT_INTRO,
        STATE_TITLE,
        STATE_BRIEFING,
        STATE_PLAYING,
        STATE_BRIEFING,
        STATE_PLANET_SELECT,
        STATE_PLAYING,
    ];
    let seq_no_boot: Vec<u8> = seq
        .iter()
        .copied()
        .filter(|&state| state != STATE_BOOT)
        .collect();
    assert!(
        seq_no_boot.starts_with(expected_tail),
        "state sequence {seq_no_boot:?} does not start with \
         ATTRACT_INTRO->TITLE->BRIEFING->TRAINING->BRIEFING->PLANET_SELECT->PLAYING"
    );

    // Title screen must have produced draw entries (logo map objects).
    let title_draws: u32 = states
        .iter()
        .filter(|(state, _)| *state == STATE_TITLE)
        .map(|(_, n)| *n)
        .max()
        .unwrap_or(0);
    assert!(
        title_draws > 0,
        "no draw-list entries during the title state"
    );

    // Frame readback exists and is a plausible PPM.
    let ppm = std::fs::read(&ppm_path).expect("PPM readback missing");
    assert!(ppm.starts_with(b"P6\n"), "not a P6 PPM");
    assert!(ppm.len() > 1000, "PPM suspiciously small");

    eprintln!(
        "smoke: dump at {}, frame at {}",
        dump_path.display(),
        ppm_path.display()
    );
}

#[test]
fn sf2_autoplay_reaches_strategic_map_and_renders_native_ui() {
    const EXPECTED_TICKS: &str = "220";
    const MAP_CAPTURE_TICK: &str = "200";
    const MIN_DISTINCT_COLORS: usize = 8;

    let _serial = SMOKE_LOCK.lock().unwrap();
    if std::env::var("DISPLAY").unwrap_or_default().is_empty()
        && std::env::var("WAYLAND_DISPLAY")
            .unwrap_or_default()
            .is_empty()
    {
        eprintln!("smoke: no display server — skipping SF2 app smoke test");
        return;
    }

    let root = repo_root();
    let rom = root.join("Star Fox 2 (USA, Europe).sfc");
    if !rom.is_file() {
        eprintln!("smoke: retail SF2 ROM absent — skipping SF2 app smoke test");
        return;
    }

    let ppm_path = scratch_dir().join("sf2_native_map.ppm");
    let output = Command::new(env!("CARGO_BIN_EXE_starfox-hd-rs"))
        .current_dir(&root)
        .arg("--sf2")
        .env("SF_AUTOPLAY", "1")
        .env("SF_FAST_FORWARD", "1")
        .env("SF_HIDDEN", "1")
        .env("SF_MAX_TICKS", EXPECTED_TICKS)
        .env("SF_DUMP_PPM", &ppm_path)
        .env("SF_DUMP_PPM_TICK", MAP_CAPTURE_TICK)
        .output()
        .expect("failed to launch native SF2 app");
    assert!(
        output.status.success(),
        "SF2 app failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for transition in [
        "[sf2-state] Intro(TitleSplash) -> Title",
        "[sf2-state] Title -> Briefing",
        "[sf2-state] Briefing -> StrategicMap",
    ] {
        assert!(
            stdout.contains(transition),
            "missing SF2 transition {transition:?} in:\n{stdout}"
        );
    }

    let (distinct_colors, _) = ppm_color_stats(&ppm_path);
    assert!(
        distinct_colors >= MIN_DISTINCT_COLORS,
        "SF2 strategic-map frame collapsed to {} colors",
        distinct_colors
    );
}

#[test]
fn sf2_autoplay_launches_opening_sortie_and_renders_native_mission() {
    const EXPECTED_TICKS: &str = "1315";
    const MISSION_CAPTURE_TICK: &str = "1313";
    const MIN_DISTINCT_COLORS: usize = 14;
    const MIN_VISIBLE_PIXELS: usize = 10_000;
    const MIN_RENDER_OBJECTS: usize = 6;
    const ENEMY_LASER_SHAPE_ID: u16 = sf_core::shape::sf2_shape_id(357);
    const EXPECTED_ENEMY_LASERS: usize = 2;

    let _serial = SMOKE_LOCK.lock().unwrap();
    if std::env::var("DISPLAY").unwrap_or_default().is_empty()
        && std::env::var("WAYLAND_DISPLAY")
            .unwrap_or_default()
            .is_empty()
    {
        eprintln!("smoke: no display server — skipping SF2 mission smoke test");
        return;
    }

    let root = repo_root();
    let rom = root.join("Star Fox 2 (USA, Europe).sfc");
    if !rom.is_file() {
        eprintln!("smoke: retail SF2 ROM absent — skipping SF2 mission smoke test");
        return;
    }

    let ppm_path = scratch_dir().join("sf2_native_opening_sortie.ppm");
    let dump_path = scratch_dir().join("sf2_native_opening_sortie_state.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_starfox-hd-rs"))
        .current_dir(&root)
        .arg("--sf2")
        .env("SF_AUTOPLAY", "1")
        .env("SF_FAST_FORWARD", "1")
        .env("SF_HIDDEN", "1")
        .env("SF_MAX_TICKS", EXPECTED_TICKS)
        .env("SF_STATE_DUMP", &dump_path)
        .env("SF_DUMP_PPM", &ppm_path)
        .env("SF_DUMP_PPM_TICK", MISSION_CAPTURE_TICK)
        .output()
        .expect("failed to launch native SF2 mission");
    assert!(
        output.status.success(),
        "SF2 mission failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for transition in [
        "[sf2-state] Intro(TitleSplash) -> Title",
        "[sf2-state] Title -> Briefing",
        "[sf2-state] Briefing -> StrategicMap",
        "[sf2-state] StrategicMap -> PilotSelection",
        "[sf2-state] PilotSelection -> StrategicMap",
        "[sf2-state] StrategicMap -> Mission",
    ] {
        assert!(
            stdout.contains(transition),
            "missing SF2 transition {transition:?} in:\n{stdout}"
        );
    }

    let dump = std::fs::read_to_string(&dump_path).expect("SF2 mission state dump is missing");
    let capture_header = format!("T {MISSION_CAPTURE_TICK} ");
    let render_object_count = dump
        .lines()
        .find(|line| line.starts_with(&capture_header))
        .and_then(|line| line.split_ascii_whitespace().nth(4))
        .and_then(|count| count.parse::<usize>().ok())
        .expect("SF2 mission capture tick is absent from the state dump");
    assert!(
        render_object_count >= MIN_RENDER_OBJECTS,
        "SF2 mission has only {render_object_count} render objects at capture"
    );
    let enemy_laser_count = dump
        .lines()
        .skip_while(|line| !line.starts_with(&capture_header))
        .skip(1)
        .take_while(|line| line.starts_with("E "))
        .filter(|entry| {
            entry
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|token| token.parse::<u16>().ok())
                == Some(ENEMY_LASER_SHAPE_ID)
        })
        .count();
    assert_eq!(
        enemy_laser_count, EXPECTED_ENEMY_LASERS,
        "opening encounter did not expose both retail enemy lasers"
    );

    let (distinct_colors, visible_pixels) = ppm_color_stats(&ppm_path);
    assert!(
        distinct_colors >= MIN_DISTINCT_COLORS,
        "SF2 mission frame collapsed to {distinct_colors} colors"
    );
    assert!(
        visible_pixels >= MIN_VISIBLE_PIXELS,
        "SF2 mission frame has only {visible_pixels} visible pixels"
    );
}

#[test]
fn sf2_active_laser_respects_the_native_render_boundary() {
    const EXPECTED_TICKS: &str = "1204";
    const LASER_CAPTURE_TICK: &str = "1203";
    const CAPTURE_WIDTH: usize = 1280;
    const CAPTURE_HEIGHT: usize = 720;
    const SOURCE_VIEWPORT_LEFT: usize = 228;
    const SOURCE_VIEWPORT_RIGHT: usize = 1_051;
    const CLEAR_COLOR: [u8; 3] = [0, 0, 13];
    const HIDDEN_PLAYER_CRAFT_SHAPE_ID: u16 =
        sf2_game::ShapeId::FOX_FALCO_FLIGHT_CRAFT.flat_render_id();
    const LASER_SHAPE_IDS: [u16; 8] = [
        sf2_game::ShapeId::PLAYER_RAPID_LASER_LAUNCH.flat_render_id(),
        sf2_game::ShapeId::PLAYER_RAPID_LASER_EXPANDED.flat_render_id(),
        sf2_game::ShapeId::PLAYER_RAPID_LASER_FAST.flat_render_id(),
        sf2_game::ShapeId::PLAYER_RAPID_LASER_DISTANT.flat_render_id(),
        sf2_game::ShapeId::PLAYER_CHARGE_ORB_BUILDING.flat_render_id(),
        sf2_game::ShapeId::PLAYER_CHARGE_ORB_READY.flat_render_id(),
        sf2_game::ShapeId::PLAYER_CHARGED_LASER_LAUNCH.flat_render_id(),
        sf2_game::ShapeId::PLAYER_CHARGED_LASER_ACTIVE.flat_render_id(),
    ];

    let _serial = SMOKE_LOCK.lock().unwrap();
    if std::env::var("DISPLAY").unwrap_or_default().is_empty()
        && std::env::var("WAYLAND_DISPLAY")
            .unwrap_or_default()
            .is_empty()
    {
        eprintln!("smoke: no display server — skipping SF2 active-flight smoke test");
        return;
    }

    let root = repo_root();
    let rom = root.join("Star Fox 2 (USA, Europe).sfc");
    if !rom.is_file() {
        eprintln!("smoke: retail SF2 ROM absent — skipping SF2 active-flight smoke test");
        return;
    }

    let ppm_path = scratch_dir().join("sf2_native_active_laser.ppm");
    let dump_path = scratch_dir().join("sf2_native_active_laser_state.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_starfox-hd-rs"))
        .current_dir(&root)
        .arg("--sf2")
        .env("SF_AUTOPLAY", "1")
        .env("SF_FAST_FORWARD", "1")
        .env("SF_HIDDEN", "1")
        .env("SF_MAX_TICKS", EXPECTED_TICKS)
        .env("SF_STATE_DUMP", &dump_path)
        .env("SF_DUMP_PPM", &ppm_path)
        .env("SF_DUMP_PPM_TICK", LASER_CAPTURE_TICK)
        .output()
        .expect("failed to launch native SF2 active flight");
    assert!(
        output.status.success(),
        "SF2 active flight failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dump = std::fs::read_to_string(&dump_path).expect("SF2 state dump is missing");
    let capture_header = format!("T {LASER_CAPTURE_TICK} ");
    let capture_entries: Vec<_> = dump
        .lines()
        .skip_while(|line| !line.starts_with(&capture_header))
        .skip(1)
        .take_while(|line| line.starts_with("E "))
        .collect();
    assert!(
        capture_entries.iter().all(|entry| {
            entry
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|token| token.parse::<u16>().ok())
                != Some(HIDDEN_PLAYER_CRAFT_SHAPE_ID)
        }),
        "SF2 first-person flight submitted the hidden player craft at tick {LASER_CAPTURE_TICK}"
    );
    assert!(
        capture_entries.iter().any(|entry| {
            entry
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|token| token.parse::<u16>().ok())
                .is_some_and(|shape_id| LASER_SHAPE_IDS.contains(&shape_id))
        }),
        "SF2 laser has no render entry at capture tick {LASER_CAPTURE_TICK}"
    );

    let (width, height, pixels) = ppm_rgb(&ppm_path);
    assert_eq!((width, height), (CAPTURE_WIDTH, CAPTURE_HEIGHT));
    let leaked_pixels = (0..height)
        .flat_map(|y| {
            (0..SOURCE_VIEWPORT_LEFT)
                .chain(SOURCE_VIEWPORT_RIGHT..width)
                .map(move |x| (x, y))
        })
        .filter(|&(x, y)| {
            let offset = (y * width + x) * 3;
            pixels[offset..offset + 3] != CLEAR_COLOR
        })
        .count();
    assert_eq!(leaked_pixels, 0, "SF2 geometry leaked into the side bars");
}

/// Optional differential check against the C oracle trace: set
/// SF_C_STATE_DUMP=<path to a C-oracle SF_STATE_DUMP file> to compare the
/// state-transition sequence and report the first draw-list divergence
/// tick (baseline metric; object parity lands with the strategy lane).
#[test]
fn state_sequence_matches_c_oracle_if_provided() {
    let Ok(c_dump_path) = std::env::var("SF_C_STATE_DUMP") else {
        eprintln!("smoke: SF_C_STATE_DUMP not set — skipping oracle diff");
        return;
    };
    let Ok(rs_dump_path) = std::env::var("SF_RS_STATE_DUMP") else {
        eprintln!("smoke: SF_RS_STATE_DUMP not set — skipping oracle diff");
        return;
    };
    let c_dump = std::fs::read_to_string(Path::new(&c_dump_path)).expect("C dump");
    let rs_dump = std::fs::read_to_string(Path::new(&rs_dump_path)).expect("Rust dump");
    let c_states = parse_states(&c_dump);
    let rs_states = parse_states(&rs_dump);

    let c_seq = dedup_states(&c_states);
    let rs_seq = dedup_states(&rs_states);
    let n = c_seq.len().min(rs_seq.len());
    assert_eq!(
        &c_seq[..n],
        &rs_seq[..n],
        "state transition sequences diverge: C {c_seq:?} vs Rust {rs_seq:?}"
    );

    // First tick where the draw-list entry count differs (baseline metric).
    let first_div = c_states
        .iter()
        .zip(rs_states.iter())
        .position(|((_, cn), (_, rn))| cn != rn);
    match first_div {
        Some(t) => eprintln!(
            "smoke: first draw-count divergence at tick {t} (C {} vs Rust {})",
            c_states[t].1, rs_states[t].1
        ),
        None => eprintln!("smoke: no draw-count divergence over the compared window"),
    }
}
