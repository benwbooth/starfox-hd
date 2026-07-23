//! Offline oracle renderer for the shipping native-audio asset cache.
//!
//! Build with `--features oracle-audio`. The generated WAV files contain the
//! original driver's mixed output, but the shipping player only sees PCM.

use sf_audio::boot;
use sf_audio::player::SpcPlayer;
use std::path::{Path, PathBuf};

const SAMPLE_RATE: usize = 32_000;
const RENDER_FRAMES: usize = 1_024;
const DEFAULT_MUSIC_SECONDS: usize = 180;
const DEFAULT_EFFECT_SECONDS: usize = 3;
const DEFAULT_LOOP_SECONDS: usize = 4;
const CONTROL_PROBE_WARMUP_SECONDS: usize = 5;
const CONTROL_PROBE_WINDOW_FRAMES: usize = SAMPLE_RATE / 10;
const EFFECT_CHANNEL: u8 = 3;
const ENGINE_CHANNEL: u8 = 1;
const AMBIENCE_CHANNEL: u8 = 2;
const REFERENCE_SOUND_TRACK: u8 = boot::SND_11;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().unwrap_or_else(|| "music".to_string());
    let asset_dir = std::env::var_os("SF_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    let seconds = arguments.next().and_then(|value| value.parse().ok());
    let selection = arguments.next();

    match mode.as_str() {
        "music" => render_music(
            &asset_dir,
            seconds.unwrap_or(DEFAULT_MUSIC_SECONDS),
            selection.as_deref(),
        )?,
        "music-cues" => render_music_cues(
            &asset_dir,
            seconds.unwrap_or(DEFAULT_MUSIC_SECONDS),
            selection
                .as_deref()
                .ok_or("music-cues requires track:cue pairs")?,
        )?,
        "effects" => render_channel_range(
            &asset_dir,
            "effects",
            EFFECT_CHANNEL,
            seconds.unwrap_or(DEFAULT_EFFECT_SECONDS),
            selection.as_deref(),
            false,
        )?,
        "engine" => render_channel_range(
            &asset_dir,
            "engine",
            ENGINE_CHANNEL,
            seconds.unwrap_or(DEFAULT_LOOP_SECONDS),
            selection.as_deref(),
            true,
        )?,
        "ambience" => render_channel_range(
            &asset_dir,
            "ambience",
            AMBIENCE_CHANNEL,
            seconds.unwrap_or(DEFAULT_LOOP_SECONDS),
            selection.as_deref(),
            true,
        )?,
        "probe-control" => probe_control(
            &asset_dir,
            seconds.unwrap_or(DEFAULT_EFFECT_SECONDS),
            selection
                .as_deref()
                .ok_or("probe-control requires track:start-cue:control")?,
        )?,
        _ => {
            return Err(format!(
                "unknown mode {mode}; expected music, music-cues, effects, engine, ambience, or probe-control"
            )
            .into())
        }
    }
    Ok(())
}

/// Measure a source-driver control command after a stable music warmup.
/// This is verification output only; it never becomes a shipping runtime.
fn probe_control(
    asset_dir: &Path,
    seconds: usize,
    selection: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fields = selection.split(':');
    let track = parse_number(fields.next().ok_or("missing track")?)?;
    let start_cue = parse_number(fields.next().ok_or("missing start cue")?)?;
    let control = parse_number(fields.next().ok_or("missing control")?)?;
    if fields.next().is_some() {
        return Err("probe-control accepts exactly track:start-cue:control".into());
    }

    let player = boot_player(asset_dir, track)?;
    let baseline_player = boot_player(asset_dir, track)?;
    player.start_bgm(start_cue);
    baseline_player.start_bgm(start_cue);
    let _ = render(&player, CONTROL_PROBE_WARMUP_SECONDS);
    let _ = render(&baseline_player, CONTROL_PROBE_WARMUP_SECONDS);
    player.start_bgm(control);
    let samples = render(&player, seconds);
    let baseline = render(&baseline_player, seconds);
    let last_nonzero_frame = samples
        .chunks_exact(2)
        .rposition(|frame| frame[0] != 0 || frame[1] != 0);
    println!("last nonzero frame: {last_nonzero_frame:?}");

    for (window, stereo_samples) in samples.chunks(CONTROL_PROBE_WINDOW_FRAMES * 2).enumerate() {
        let baseline_start = window * CONTROL_PROBE_WINDOW_FRAMES * 2;
        let baseline_end = baseline_start + stereo_samples.len();
        let rms = root_mean_square(stereo_samples);
        let baseline_rms = root_mean_square(&baseline[baseline_start..baseline_end]);
        let gain = if baseline_rms == 0.0 {
            0.0
        } else {
            rms / baseline_rms
        };
        println!("window {window:03}: rms {rms:.2} relative_gain {gain:.4}");
    }
    Ok(())
}

fn root_mean_square(samples: &[i16]) -> f64 {
    let sum_squares = samples
        .iter()
        .map(|sample| {
            let sample = i64::from(*sample);
            sample * sample
        })
        .sum::<i64>();
    (sum_squares as f64 / samples.len() as f64).sqrt()
}

fn parse_selection(selection: Option<&str>, inclusive_end: u8) -> Result<Vec<u8>, String> {
    let Some(selection) = selection else {
        return Ok((0..=inclusive_end).collect());
    };
    selection
        .split(',')
        .map(|part| {
            let part = part.trim();
            if let Some(hex) = part.strip_prefix("0x") {
                u8::from_str_radix(hex, 16).map_err(|error| error.to_string())
            } else {
                part.parse::<u8>().map_err(|error| error.to_string())
            }
        })
        .collect()
}

fn render_music(
    asset_dir: &Path,
    seconds: usize,
    selection: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tracks = parse_selection(selection, boot::SND_TRACK_COUNT - 1)?;
    let output_dir = asset_dir.join("native_audio/music");
    std::fs::create_dir_all(&output_dir)?;
    for track in tracks {
        let cue = boot::track_command(track);
        let player = boot_player(asset_dir, track)?;
        player.start_bgm(cue);
        let samples = render(&player, seconds);
        let path = output_dir.join(format!("track_{track:02}_cue_{cue:02X}.wav"));
        write_wave(&path, &samples)?;
        println!("rendered {}", path.display());
    }
    Ok(())
}

fn render_music_cues(
    asset_dir: &Path,
    seconds: usize,
    selection: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = asset_dir.join("native_audio/music");
    std::fs::create_dir_all(&output_dir)?;
    for pair in selection.split(',') {
        let (track, cue) = pair
            .trim()
            .split_once(':')
            .ok_or_else(|| format!("invalid track:cue pair {pair}"))?;
        let track = parse_number(track)?;
        let cue = parse_number(cue)?;
        let player = boot_player(asset_dir, track)?;
        player.start_bgm(cue);
        let samples = render(&player, seconds);
        let path = output_dir.join(format!("track_{track:02}_cue_{cue:02X}.wav"));
        write_wave(&path, &samples)?;
        println!("rendered {}", path.display());
    }
    Ok(())
}

fn parse_number(value: &str) -> Result<u8, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u8::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn render_channel_range(
    asset_dir: &Path,
    directory: &str,
    channel: u8,
    seconds: usize,
    selection: Option<&str>,
    looping: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sounds = parse_selection(selection, u8::MAX)?;
    let output_dir = asset_dir.join("native_audio").join(directory);
    std::fs::create_dir_all(&output_dir)?;
    for sound in sounds.into_iter().filter(|sound| *sound != 0) {
        let player = boot_player(asset_dir, REFERENCE_SOUND_TRACK)?;
        player.write_port(channel, sound);
        let samples = render(&player, seconds);
        let path = output_dir.join(format!("{sound:02X}.wav"));
        write_wave(&path, &samples)?;
        if looping {
            let loop_start = SAMPLE_RATE * (seconds / 2);
            std::fs::write(path.with_extension("loop"), loop_start.to_string())?;
        }
        println!("rendered {}", path.display());
    }
    Ok(())
}

fn boot_player(asset_dir: &Path, track: u8) -> Result<SpcPlayer, Box<dyn std::error::Error>> {
    let player = SpcPlayer::new();
    player.load_track(track, asset_dir)?;
    Ok(player)
}

fn render(player: &SpcPlayer, seconds: usize) -> Vec<i16> {
    let total_frames = seconds * SAMPLE_RATE;
    let mut output = Vec::with_capacity(total_frames * 2);
    let mut buffer = vec![0i16; RENDER_FRAMES * 2];
    while output.len() < total_frames * 2 {
        player.generate(&mut buffer);
        let remaining = total_frames * 2 - output.len();
        output.extend_from_slice(&buffer[..remaining.min(buffer.len())]);
    }
    output
}

fn write_wave(path: &Path, samples: &[i16]) -> std::io::Result<()> {
    let data_length = u32::try_from(samples.len() * 2).expect("native audio WAV under 4 GiB");
    let mut output = Vec::with_capacity(44 + samples.len() * 2);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&(36 + data_length).to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&16u32.to_le_bytes());
    output.extend_from_slice(&1u16.to_le_bytes());
    output.extend_from_slice(&2u16.to_le_bytes());
    output.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    output.extend_from_slice(&((SAMPLE_RATE * 4) as u32).to_le_bytes());
    output.extend_from_slice(&4u16.to_le_bytes());
    output.extend_from_slice(&16u16.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&data_length.to_le_bytes());
    for sample in samples {
        output.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, output)
}
