//! Offline renderer for Star Fox 2's semantic native sound assets.
//!
//! The command values and timing below come from the Mesen audio-events
//! oracle. Shipping code only loads the resulting named PCM files.

use sf2_data::{audio, oracle_audio};
use sf_audio::player::SpcPlayer;
use std::path::{Path, PathBuf};

const SAMPLE_RATE: usize = 32_000;
const EFFECT_CHANNEL: u8 = 3;
const ENGINE_CHANNEL: u8 = 1;
const RAPID_LASER_FIRST_COMMAND: u8 = 0x13;
const RAPID_LASER_SECOND_COMMAND: u8 = 0x16;
const CHARGE_BUILDING_COMMAND: u8 = 0x31;
const CHARGE_READY_COMMAND: u8 = 0x35;
const CHARGED_LASER_FIRST_COMMAND: u8 = 0xF4;
const CHARGED_LASER_SECOND_COMMAND: u8 = 0x20;
const HOSTILE_LASER_COMMAND: u8 = 0x72;
const HOSTILE_LASER_PARAMETER: u8 = 0x61;
const FLIGHT_ENGINE_COMMAND: u8 = 0x04;
const PAIRED_COMMAND_DELAY_FRAMES: usize = 464;
const BUILD_TO_READY_DELAY_FRAMES: usize = 16_459;
const READY_TO_RELEASE_DELAY_FRAMES: usize = 27_688;
const EFFECT_DURATION_SECONDS: usize = 3;
const ENGINE_DURATION_SECONDS: usize = 4;
const LOOP_SEARCH_START_SECONDS: usize = 1;
const LOOP_COMPARISON_FRAMES: usize = 512;

const SOURCE_SOUND_BANK_COUNT: usize = 8;
const SOURCE_PILOT_COUNT: usize = 6;
const SEMANTIC_SOUND_COUNT: usize = 6;

#[derive(Debug, Clone)]
struct SourceSoundBank {
    name: String,
    program_record: u16,
}

const SOURCE_SOUND_BANKS: [(&str, u16); SOURCE_SOUND_BANK_COUNT] = [
    ("open_space", 0x115),
    ("fighter_intercept", 0x129),
    ("titania", 0x076),
    ("eladard", 0x09E),
    ("carrier", 0x13D),
    ("mirage", 0x151),
    ("rival", 0x173),
    ("astropolis", 0x0E2),
];

#[derive(Debug, Clone, Copy)]
struct SourcePilot {
    name: &'static str,
    index: usize,
}

const SOURCE_PILOTS: [SourcePilot; SOURCE_PILOT_COUNT] = [
    SourcePilot {
        name: "fox",
        index: 0,
    },
    SourcePilot {
        name: "falco",
        index: 1,
    },
    SourcePilot {
        name: "peppy",
        index: 2,
    },
    SourcePilot {
        name: "slippy",
        index: 3,
    },
    SourcePilot {
        name: "miyu",
        index: 4,
    },
    SourcePilot {
        name: "fay",
        index: 5,
    },
];

#[derive(Debug, Clone, Copy)]
enum SemanticSound {
    RapidLaser,
    ChargeBuilding,
    ChargeReady,
    ChargedLaser,
    HostileLaser,
    FlightEngine,
}

impl SemanticSound {
    const ALL: [Self; SEMANTIC_SOUND_COUNT] = [
        Self::RapidLaser,
        Self::ChargeBuilding,
        Self::ChargeReady,
        Self::ChargedLaser,
        Self::HostileLaser,
        Self::FlightEngine,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::RapidLaser => "rapid_laser",
            Self::ChargeBuilding => "charge_building",
            Self::ChargeReady => "charge_ready",
            Self::ChargedLaser => "charged_laser",
            Self::HostileLaser => "hostile_laser",
            Self::FlightEngine => "flight",
        }
    }

    const fn commands(self) -> &'static [(u8, u8)] {
        match self {
            Self::RapidLaser => &[
                (EFFECT_CHANNEL, RAPID_LASER_FIRST_COMMAND),
                (EFFECT_CHANNEL, RAPID_LASER_SECOND_COMMAND),
            ],
            Self::ChargeBuilding => &[(EFFECT_CHANNEL, CHARGE_BUILDING_COMMAND)],
            Self::ChargeReady => &[(EFFECT_CHANNEL, CHARGE_READY_COMMAND)],
            Self::ChargedLaser => &[
                (EFFECT_CHANNEL, CHARGED_LASER_FIRST_COMMAND),
                (EFFECT_CHANNEL, CHARGED_LASER_SECOND_COMMAND),
            ],
            Self::HostileLaser => &[
                (EFFECT_CHANNEL, HOSTILE_LASER_COMMAND),
                (0, HOSTILE_LASER_PARAMETER),
            ],
            Self::FlightEngine => &[(ENGINE_CHANNEL, FLIGHT_ENGINE_COMMAND)],
        }
    }

    const fn duration_seconds(self) -> usize {
        match self {
            Self::FlightEngine => ENGINE_DURATION_SECONDS,
            Self::RapidLaser
            | Self::ChargeBuilding
            | Self::ChargeReady
            | Self::ChargedLaser
            | Self::HostileLaser => EFFECT_DURATION_SECONDS,
        }
    }

    const fn directory(self) -> &'static str {
        match self {
            Self::FlightEngine => "engine",
            Self::ChargeBuilding | Self::ChargeReady => "ambience",
            Self::RapidLaser | Self::ChargedLaser | Self::HostileLaser => "effects",
        }
    }

    const fn looping(self) -> bool {
        match self {
            Self::FlightEngine => true,
            Self::RapidLaser
            | Self::ChargeBuilding
            | Self::ChargeReady
            | Self::ChargedLaser
            | Self::HostileLaser => false,
        }
    }

    const fn minimum_rms(self) -> f64 {
        match self {
            Self::RapidLaser | Self::ChargeBuilding | Self::FlightEngine => 20.0,
            Self::ChargeReady => 1.0,
            Self::ChargedLaser => 0.01,
            Self::HostileLaser => 1.0,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selection = std::env::args().nth(1);
    let sounds = parse_selection(selection.as_deref())?;
    let asset_dir = std::env::var_os("SF2_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/sf2"));
    let source_banks = selected_sound_banks()?;
    let source_pilots = selected_pilots()?;
    let output_root = std::env::var_os("SF2_NATIVE_SOUND_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| asset_dir.join("../native_audio_sf2"));

    for bank in &source_banks {
        for pilot in source_pilots.iter().copied() {
            for sound in sounds.iter().copied() {
                let player = boot_program(&asset_dir, bank.program_record, pilot.index)?;
                let samples = render_sound(&player, sound);
                let rms = root_mean_square(&samples);
                if rms < sound.minimum_rms() {
                    return Err(format!(
                        "{} {}/{} rendered below its verified response threshold (rms {rms:.2}, minimum {:.2})",
                        sound.name(),
                        bank.name,
                        pilot.name,
                        sound.minimum_rms(),
                    )
                    .into());
                }
                let output_dir = output_root
                    .join(sound.directory())
                    .join(&bank.name)
                    .join(pilot.name);
                std::fs::create_dir_all(&output_dir)?;
                let path = output_dir.join(format!("{}.wav", sound.name()));
                write_wave(&path, &samples)?;
                if sound.looping() {
                    std::fs::write(
                        path.with_extension("loop"),
                        best_loop_start(&samples).to_string(),
                    )?;
                }
                let peak = samples
                    .iter()
                    .map(|sample| sample.unsigned_abs())
                    .max()
                    .unwrap_or_default();
                println!(
                    "{}/{} {}: source {:#05X}, commands {:?}, rms {rms:.2}, peak {peak}, {}",
                    bank.name,
                    pilot.name,
                    sound.name(),
                    bank.program_record,
                    sound.commands(),
                    path.display(),
                );
            }
        }
    }
    Ok(())
}

fn selected_sound_banks() -> Result<Vec<SourceSoundBank>, Box<dyn std::error::Error>> {
    let Some(value) = std::env::var("SF2_SOUND_PROGRAM_RECORD").ok() else {
        return Ok(SOURCE_SOUND_BANKS
            .iter()
            .map(|(name, program_record)| SourceSoundBank {
                name: (*name).to_owned(),
                program_record: *program_record,
            })
            .collect());
    };
    let name = std::env::var("SF2_SOUND_BANK_NAME").unwrap_or_else(|_| "probe".to_owned());
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err("SF2_SOUND_BANK_NAME must be one safe directory component".into());
    }
    Ok(vec![SourceSoundBank {
        name,
        program_record: parse_number(&value)?,
    }])
}

fn selected_pilots() -> Result<Vec<SourcePilot>, Box<dyn std::error::Error>> {
    let Some(value) = std::env::var("SF2_SOUND_PILOT_INDEX").ok() else {
        return Ok(SOURCE_PILOTS.to_vec());
    };
    let index: usize = value.parse()?;
    let pilot = SOURCE_PILOTS
        .get(index)
        .copied()
        .ok_or_else(|| format!("pilot index {index} is outside the retail table"))?;
    Ok(vec![pilot])
}

fn parse_selection(selection: Option<&str>) -> Result<Vec<SemanticSound>, String> {
    let Some(selection) = selection else {
        return Ok(SemanticSound::ALL.to_vec());
    };
    selection
        .split(',')
        .map(|name| match name.trim() {
            "rapid-laser" => Ok(SemanticSound::RapidLaser),
            "charge-building" => Ok(SemanticSound::ChargeBuilding),
            "charge-ready" => Ok(SemanticSound::ChargeReady),
            "charged-laser" => Ok(SemanticSound::ChargedLaser),
            "hostile-laser" => Ok(SemanticSound::HostileLaser),
            "flight-engine" => Ok(SemanticSound::FlightEngine),
            unknown => Err(format!("unknown semantic sound {unknown}")),
        })
        .collect()
}

fn parse_number(value: &str) -> Result<u16, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u16::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn boot_program(
    asset_dir: &Path,
    source_program_record: u16,
    source_pilot_index: usize,
) -> Result<SpcPlayer, Box<dyn std::error::Error>> {
    let program = oracle_audio::UPLOAD_PROGRAMS
        .iter()
        .find(|program| program.source_record_offset == source_program_record)
        .ok_or("selected audio program is missing")?;
    let mut files =
        Vec::with_capacity(2 + oracle_audio::BASE_BOOT_BLOB_IDS.len() + program.blob_ids.len());
    files.push(audio::DRIVER_UPLOAD_FILE);
    files.extend(
        oracle_audio::BASE_BOOT_BLOB_IDS
            .iter()
            .map(|blob| audio::AUDIO_BLOBS[usize::from(*blob)].file),
    );
    files.extend(
        program
            .blob_ids
            .iter()
            .map(|blob| audio::AUDIO_BLOBS[usize::from(*blob)].file),
    );
    let pilot_blob = oracle_audio::PILOT_BLOB_IDS[source_pilot_index];
    files.push(audio::AUDIO_BLOBS[usize::from(pilot_blob)].file);
    let player = SpcPlayer::new();
    player.load_files(&files, asset_dir, audio::SPC_DRIVER_ENTRY)?;
    Ok(player)
}

fn render_sound(player: &SpcPlayer, sound: SemanticSound) -> Vec<i16> {
    let total_frames = sound.duration_seconds() * SAMPLE_RATE;
    match sound {
        SemanticSound::ChargeReady => {
            player.write_port(EFFECT_CHANNEL, CHARGE_BUILDING_COMMAND);
            discard_frames(player, BUILD_TO_READY_DELAY_FRAMES);
            player.write_port(EFFECT_CHANNEL, CHARGE_READY_COMMAND);
            let mut output = Vec::with_capacity(total_frames * 2);
            append_frames(player, &mut output, total_frames);
            return output;
        }
        SemanticSound::ChargedLaser => {
            player.write_port(EFFECT_CHANNEL, CHARGE_BUILDING_COMMAND);
            discard_frames(player, BUILD_TO_READY_DELAY_FRAMES);
            player.write_port(EFFECT_CHANNEL, CHARGE_READY_COMMAND);
            discard_frames(player, READY_TO_RELEASE_DELAY_FRAMES);
            let mut output = Vec::with_capacity(total_frames * 2);
            player.write_port(EFFECT_CHANNEL, CHARGED_LASER_FIRST_COMMAND);
            append_frames(player, &mut output, PAIRED_COMMAND_DELAY_FRAMES);
            player.write_port(EFFECT_CHANNEL, CHARGED_LASER_SECOND_COMMAND);
            append_frames(
                player,
                &mut output,
                total_frames - PAIRED_COMMAND_DELAY_FRAMES,
            );
            return output;
        }
        SemanticSound::HostileLaser => {
            let mut output = Vec::with_capacity(total_frames * 2);
            player.write_port(EFFECT_CHANNEL, HOSTILE_LASER_COMMAND);
            player.write_port(0, HOSTILE_LASER_PARAMETER);
            append_frames(player, &mut output, total_frames);
            return output;
        }
        SemanticSound::RapidLaser | SemanticSound::ChargeBuilding | SemanticSound::FlightEngine => {
        }
    }
    let commands = sound.commands();
    let mut output = Vec::with_capacity(total_frames * 2);
    for (index, (channel, command)) in commands.iter().copied().enumerate() {
        player.write_port(channel, command);
        if index + 1 < commands.len() {
            append_frames(player, &mut output, PAIRED_COMMAND_DELAY_FRAMES);
        }
    }
    let rendered_frames = output.len() / 2;
    append_frames(player, &mut output, total_frames - rendered_frames);
    output
}

fn discard_frames(player: &SpcPlayer, frame_count: usize) {
    let mut discarded = vec![0i16; frame_count * 2];
    player.generate(&mut discarded);
}

fn append_frames(player: &SpcPlayer, output: &mut Vec<i16>, frame_count: usize) {
    let mut buffer = vec![0i16; frame_count * 2];
    player.generate(&mut buffer);
    output.extend_from_slice(&buffer);
}

fn root_mean_square(samples: &[i16]) -> f64 {
    let sum = samples
        .iter()
        .map(|sample| {
            let value = i64::from(*sample);
            value * value
        })
        .sum::<i64>();
    (sum as f64 / samples.len() as f64).sqrt()
}

fn best_loop_start(samples: &[i16]) -> usize {
    let frame_count = samples.len() / 2;
    let comparison_frames = LOOP_COMPARISON_FRAMES.min(frame_count / 4);
    let target_start = frame_count - comparison_frames;
    let search_start = (LOOP_SEARCH_START_SECONDS * SAMPLE_RATE).min(target_start);
    let search_end = target_start.saturating_sub(comparison_frames);
    let mut best_start = search_start;
    let mut best_error = u64::MAX;
    for candidate in search_start..=search_end {
        let mut error = 0u64;
        for offset in 0..comparison_frames {
            let candidate_index = (candidate + offset) * 2;
            let target_index = (target_start + offset) * 2;
            for channel in 0..2 {
                let difference = i64::from(samples[candidate_index + channel])
                    - i64::from(samples[target_index + channel]);
                error = error.saturating_add(difference.unsigned_abs());
            }
        }
        if error < best_error {
            best_error = error;
            best_start = candidate;
        }
    }
    best_start
}

fn write_wave(path: &Path, samples: &[i16]) -> std::io::Result<()> {
    const HEADER_LENGTH: usize = 44;
    const FORMAT_CHUNK_LENGTH: u32 = 16;
    const PCM_ENCODING: u16 = 1;
    const CHANNEL_COUNT: u16 = 2;
    const BITS_PER_SAMPLE: u16 = 16;
    const BYTES_PER_FRAME: u16 = 4;

    let data_length = u32::try_from(samples.len() * 2).expect("SF2 oracle WAV under 4 GiB");
    let mut output = Vec::with_capacity(HEADER_LENGTH + data_length as usize);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&(36 + data_length).to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&FORMAT_CHUNK_LENGTH.to_le_bytes());
    output.extend_from_slice(&PCM_ENCODING.to_le_bytes());
    output.extend_from_slice(&CHANNEL_COUNT.to_le_bytes());
    output.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    output.extend_from_slice(&((SAMPLE_RATE * usize::from(BYTES_PER_FRAME)) as u32).to_le_bytes());
    output.extend_from_slice(&BYTES_PER_FRAME.to_le_bytes());
    output.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&data_length.to_le_bytes());
    for sample in samples {
        output.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, output)
}
