//! Smoke test: run the built binary headless-ish (hidden window) under
//! SF_AUTOPLAY for 600 ticks (~30 s of game time), dump the state trace and
//! one frame readback, and assert the boot.h state-machine transitions:
//! BOOT(0) -> TITLE(1) -> PLANET_SELECT(3) -> PLAYING(4).
//!
//! Skips (with a message) when DISPLAY is not set. The C-oracle comparison
//! (same env against build/starfox-hd) is a separate manual/differential
//! step — object-level parity is not asserted here because the strategy
//! lane has not landed.

use std::path::{Path, PathBuf};
use std::process::Command;

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

#[test]
fn autoplay_reaches_gameplay() {
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
    // boot.h: BOOT=0 TITLE=1 PLANET_SELECT=3 PLAYING=4. The first dumped
    // tick already shows TITLE (Game_Init runs inside tick 0), so accept a
    // leading 0 or 1.
    let expected_tail: &[u8] = &[1, 3, 4];
    let seq_no_boot: Vec<u8> = seq.iter().copied().filter(|&s| s != 0).collect();
    assert!(
        seq_no_boot.starts_with(expected_tail),
        "state sequence {seq_no_boot:?} does not start with TITLE->PLANET_SELECT->PLAYING"
    );

    // Title screen must have produced draw entries (logo map objects).
    let title_draws: u32 = states
        .iter()
        .filter(|(s, _)| *s == 1)
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
