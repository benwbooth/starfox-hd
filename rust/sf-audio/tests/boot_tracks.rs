#![cfg(feature = "oracle-audio")]

//! Offline end-to-end verification (no SDL): boot real tracks through the
//! IPL upload path, start them via the BGM handshake, and render audio.
//!
//! Mirrors what the C oracle does at runtime: SpcBoot_LoadTrack +
//! SpcPlayer_StartBgm + repeated SpcPlayer_Generate callbacks.
//!
//! Sound data is resolved from `$SF_ASSET_DIR` or `../../data` relative to
//! CARGO_MANIFEST_DIR (the repo's `data/snd`).  If the data is absent the
//! tests skip with a note instead of failing.

use sf_audio::boot;
use sf_audio::player::SpcPlayer;
use std::collections::HashSet;
use std::path::PathBuf;

const SAMPLE_RATE: usize = 32000; // SPC native rate
const FRAMES_PER_CALLBACK: usize = 1024; // ~32 ms per generate, like SDL
const SECONDS: usize = 5;

fn asset_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SF_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    if dir.join("snd").join("SGSOUND0.BIN").is_file() {
        Some(dir)
    } else {
        eprintln!("skipping: no sound data at {}", dir.display());
        None
    }
}

struct RenderStats {
    peak: i32,
    distinct: usize,
    echoed: Option<u8>,
    samples: Vec<i16>,
}

/// Boot `track_id`, send `cue` through the handshake, render about five
/// seconds of stereo audio and collect stats.
fn boot_and_render_cue(track_id: u8, cue: u8) -> Option<RenderStats> {
    let dir = asset_dir()?;

    let player = SpcPlayer::new();
    player
        .load_track(track_id, &dir)
        .unwrap_or_else(|e| panic!("boot track {track_id}: {e}"));

    // startmus: queue the command; the "audio thread" (our generate loop)
    // performs the port-0 write/echo/clear handshake.
    player.start_bgm(cue);

    let callbacks = SECONDS * SAMPLE_RATE / FRAMES_PER_CALLBACK;
    let mut buf = vec![0i16; FRAMES_PER_CALLBACK * 2];
    let mut samples = Vec::with_capacity(callbacks * buf.len());
    let mut peak: i32 = 0;
    let mut distinct: HashSet<i16> = HashSet::new();

    for _ in 0..callbacks {
        player.generate(&mut buf);
        for &s in &buf {
            peak = peak.max((s as i32).abs());
            distinct.insert(s);
        }
        samples.extend_from_slice(&buf);
    }

    Some(RenderStats {
        peak,
        distinct: distinct.len(),
        echoed: player.last_bgm_echo(),
        samples,
    })
}

fn boot_and_render(track_id: u8) -> Option<RenderStats> {
    boot_and_render_cue(track_id, boot::track_command(track_id))
}

fn assert_cue_audible(track_id: u8, cue: u8, stats: &RenderStats) {
    // (a) the driver echoed the start command back on port 0
    assert_eq!(
        stats.echoed,
        Some(cue),
        "track {track_id}: driver never echoed start cue ${cue:02X}"
    );
    // (b) real signal level
    assert!(
        stats.peak > 1000,
        "track {track_id}: peak {} too quiet",
        stats.peak
    );
    // (c) not DC / not a stuck value
    assert!(
        stats.distinct > 100,
        "track {track_id}: only {} distinct sample values",
        stats.distinct
    );
    eprintln!(
        "track {track_id}: peak={} distinct={} echoed=${:02X}",
        stats.peak,
        stats.distinct,
        stats.echoed.unwrap()
    );
}

fn assert_audible(track_id: u8, stats: &RenderStats) {
    assert_cue_audible(track_id, boot::track_command(track_id), stats);
}

fn assert_shipping_pcm_prefix(path: &std::path::Path, samples: &[i16], label: &str) {
    let wave =
        std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let pcm = wave
        .get(44..)
        .unwrap_or_else(|| panic!("{} has no PCM payload", path.display()));
    let expected_bytes = samples.len() * std::mem::size_of::<i16>();
    assert!(
        pcm.len() >= expected_bytes,
        "{} is shorter than the oracle prefix",
        path.display()
    );
    for (sample_index, (actual, expected)) in samples
        .iter()
        .zip(pcm[..expected_bytes].chunks_exact(2))
        .enumerate()
    {
        assert_eq!(
            actual.to_le_bytes(),
            [expected[0], expected[1]],
            "{label} PCM diverged at sample {sample_index}"
        );
    }
}

/// Minimal RIFF/WAVE writer (16-bit stereo PCM) for optional human listens.
fn dump_wav(path: &std::path::Path, samples: &[i16]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // stereo
    out.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    out.extend_from_slice(&((SAMPLE_RATE * 2 * 2) as u32).to_le_bytes()); // byte rate
    out.extend_from_slice(&4u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, out)
}

#[test]
fn title_track_boots_and_plays() {
    // SND_TITLE: single self-contained file (SGSOUND7), start cmd $12.
    let Some(stats) = boot_and_render(boot::SND_TITLE) else {
        return;
    };
    assert_audible(boot::SND_TITLE, &stats);

    // Optional human listen: set AU_WAV_DIR to dump au_title.wav.
    if let Some(dir) = std::env::var_os("AU_WAV_DIR") {
        let path = PathBuf::from(dir).join("au_title.wav");
        dump_wav(&path, &stats.samples).expect("write wav");
        eprintln!("wrote {}", path.display());
    }
}

#[test]
fn corneria_track_boots_and_plays() {
    // SND_11 (Corneria level 1-1): multi-file upload — driver + resident
    // SGSOUND1/SGSOUND2 pre-upload, then SGSOUND3 (band, overlays
    // orchestra), SGBGMA, SGBGM1.  Start cmd $03.
    let Some(stats) = boot_and_render(boot::SND_11) else {
        return;
    };
    assert_audible(boot::SND_11, &stats);

    if let Some(dir) = std::env::var_os("AU_WAV_DIR") {
        let path = PathBuf::from(dir).join("au_corneria.wav");
        dump_wav(&path, &stats.samples).expect("write wav");
        eprintln!("wrote {}", path.display());
    }
}

#[test]
fn map_track_shipping_pcm_prefix_matches_the_oracle() {
    let Some(stats) = boot_and_render(boot::SND_MAP) else {
        return;
    };
    assert_audible(boot::SND_MAP, &stats);

    let dir = asset_dir().expect("sound data used by the oracle render");
    let path = dir.join("native_audio/music/track_05_cue_01.wav");
    assert_shipping_pcm_prefix(&path, &stats.samples, "map music");
}

#[test]
fn planet_selection_zoom_cues_match_the_oracle() {
    let dir = asset_dir().expect("sound data required for planet-selection cue verification");
    for cue in sf_audio::catalog::PLANET_SELECTION_MUSIC_CUES {
        let stats = boot_and_render_cue(boot::SND_MAP, cue)
            .expect("sound data required for planet-selection cue verification");
        assert_cue_audible(boot::SND_MAP, cue, &stats);
        let path = dir.join(format!("native_audio/music/track_05_cue_{cue:02X}.wav"));
        assert_shipping_pcm_prefix(&path, &stats.samples, "planet-selection zoom");
    }
}
