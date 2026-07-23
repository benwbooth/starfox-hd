//! Offline Star Fox 2 audio-program probe and native PCM renderer.
//!
//! This verification binary runs the original sound program. The shipping
//! application only consumes the WAV files it produces.

use sf2_data::{audio, oracle_audio};
use sf_audio::player::SpcPlayer;
use std::path::{Path, PathBuf};

const SAMPLE_RATE: usize = 32_000;
const RENDER_BUFFER_FRAMES: usize = 1_024;
const DEFAULT_RENDER_SECONDS: usize = 12;
const MINIMUM_SEMANTIC_CUE_RMS: f64 = 100.0;
const EFFECT_COMMAND_CHANNEL: u8 = 3;
const LOGO_PRESENTATION_RECORD: u16 = 0x1B5;
const FORMATION_AND_TITLE_RECORD: u16 = 0;
const ANDROSS_BRIEFING_RECORD: u16 = 0x1C3;
const STRATEGIC_MAP_RECORD: u16 = 0x036;
const PILOT_SELECTION_RECORD: u16 = 0x011;
const OPEN_SPACE_COMBAT_RECORD: u16 = 0x115;
const FIGHTER_INTERCEPT_RECORD: u16 = 0x129;
const TITANIA_BASE_RECORD: u16 = 0x076;
const ELADARD_BASE_RECORD: u16 = 0x09E;
const BATTLE_CARRIER_RECORD: u16 = 0x13D;
const MIRAGE_DRAGON_RECORD: u16 = 0x151;
const RIVAL_ENCOUNTER_RECORD: u16 = 0x173;
const ASTROPOLIS_ASSAULT_RECORD: u16 = 0x0E2;
const GAME_OVER_AND_CONTINUE_RECORD: u16 = 0x184;
const CREDITS_AND_ENDING_RECORD: u16 = 0x1DC;

#[derive(Debug, Clone, Copy)]
enum SemanticCue {
    LogoPresentation,
    FormationAndTitle,
    AndrossBriefing,
    StrategicMap,
    PilotSelection,
    OpenSpaceCombat,
    FighterIntercept,
    TitaniaBase,
    EladardBase,
    BattleCarrier,
    MirageDragon,
    RivalEncounter,
    AstropolisAssault,
    GameOverAndContinue,
    CreditsAndEnding,
}

impl SemanticCue {
    const fn record_offset(self) -> u16 {
        match self {
            Self::LogoPresentation => LOGO_PRESENTATION_RECORD,
            Self::FormationAndTitle => FORMATION_AND_TITLE_RECORD,
            Self::AndrossBriefing => ANDROSS_BRIEFING_RECORD,
            Self::StrategicMap => STRATEGIC_MAP_RECORD,
            Self::PilotSelection => PILOT_SELECTION_RECORD,
            Self::OpenSpaceCombat => OPEN_SPACE_COMBAT_RECORD,
            Self::FighterIntercept => FIGHTER_INTERCEPT_RECORD,
            Self::TitaniaBase => TITANIA_BASE_RECORD,
            Self::EladardBase => ELADARD_BASE_RECORD,
            Self::BattleCarrier => BATTLE_CARRIER_RECORD,
            Self::MirageDragon => MIRAGE_DRAGON_RECORD,
            Self::RivalEncounter => RIVAL_ENCOUNTER_RECORD,
            Self::AstropolisAssault => ASTROPOLIS_ASSAULT_RECORD,
            Self::GameOverAndContinue => GAME_OVER_AND_CONTINUE_RECORD,
            Self::CreditsAndEnding => CREDITS_AND_ENDING_RECORD,
        }
    }

    const fn filename(self) -> &'static str {
        match self {
            Self::LogoPresentation => "logo_presentation",
            Self::FormationAndTitle => "formation_and_title",
            Self::AndrossBriefing => "andross_briefing",
            Self::StrategicMap => "strategic_map",
            Self::PilotSelection => "pilot_selection",
            Self::OpenSpaceCombat => "open_space_combat",
            Self::FighterIntercept => "fighter_intercept",
            Self::TitaniaBase => "titania_base",
            Self::EladardBase => "eladard_base",
            Self::BattleCarrier => "battle_carrier",
            Self::MirageDragon => "mirage_dragon",
            Self::RivalEncounter => "rival_encounter",
            Self::AstropolisAssault => "astropolis_assault",
            Self::GameOverAndContinue => "game_over_and_continue",
            Self::CreditsAndEnding => "credits_and_ending",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ProgramSelection {
    SourceMode(usize),
    RecordOffset(u16),
    Semantic(SemanticCue),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let seconds = arguments
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(DEFAULT_RENDER_SECONDS);
    let selections = parse_selections(arguments.next().as_deref())?;
    let asset_dir = std::env::var_os("SF2_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/sf2"));
    let output_dir = asset_dir.join("../native_audio_sf2/music");
    std::fs::create_dir_all(&output_dir)?;

    for selection in selections {
        let (label, semantic_filename, program_index, resident_program_index) = match selection {
            ProgramSelection::SourceMode(mode) => (
                format!("source_mode_{mode:02}"),
                false,
                oracle_audio::SOURCE_MODE_PROGRAM_INDEX[mode],
                None,
            ),
            ProgramSelection::RecordOffset(offset) => {
                let index = oracle_audio::UPLOAD_PROGRAMS
                    .iter()
                    .position(|program| program.source_record_offset == offset)
                    .ok_or_else(|| format!("record offset {offset:03X} is not in the table"))?;
                (format!("record_{offset:03X}"), false, index, None)
            }
            ProgramSelection::Semantic(cue) => {
                let offset = cue.record_offset();
                let index = oracle_audio::UPLOAD_PROGRAMS
                    .iter()
                    .position(|program| program.source_record_offset == offset)
                    .ok_or_else(|| format!("semantic cue record {offset:03X} is missing"))?;
                let resident = match cue {
                    SemanticCue::AndrossBriefing | SemanticCue::StrategicMap => {
                        Some(program_index_for_record(FORMATION_AND_TITLE_RECORD)?)
                    }
                    SemanticCue::LogoPresentation
                    | SemanticCue::FormationAndTitle
                    | SemanticCue::PilotSelection
                    | SemanticCue::OpenSpaceCombat
                    | SemanticCue::FighterIntercept
                    | SemanticCue::TitaniaBase
                    | SemanticCue::EladardBase
                    | SemanticCue::BattleCarrier
                    | SemanticCue::MirageDragon
                    | SemanticCue::RivalEncounter
                    | SemanticCue::AstropolisAssault
                    | SemanticCue::GameOverAndContinue
                    | SemanticCue::CreditsAndEnding => None,
                };
                (cue.filename().to_string(), true, index, resident)
            }
        };
        let program = &oracle_audio::UPLOAD_PROGRAMS[program_index];
        let resident_program =
            resident_program_index.map(|index| &oracle_audio::UPLOAD_PROGRAMS[index]);
        let player = boot_program(&asset_dir, resident_program, program)?;
        if let Some(command) = program.preload_command {
            player.start_bgm(command);
        }
        player.write_port(EFFECT_COMMAND_CHANNEL, program.start_cue);
        let samples = render(&player, seconds);
        let rms = root_mean_square(&samples);
        let peak = samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0);
        if semantic_filename && rms < MINIMUM_SEMANTIC_CUE_RMS {
            return Err(format!(
                "semantic cue {label} rendered near-silence (rms {rms:.2}, minimum {MINIMUM_SEMANTIC_CUE_RMS:.2})"
            )
            .into());
        }
        let filename = if semantic_filename {
            format!("{label}.wav")
        } else {
            format!("{label}_cue_{:02X}.wav", program.start_cue)
        };
        let path = output_dir.join(filename);
        write_wave(&path, &samples)?;
        println!(
            "{label}: record {:03X}, preload {:?}, cue {:02X}, blobs {:?}, rms {rms:.2}, peak {peak}, {}",
            program.source_record_offset,
            program.preload_command,
            program.start_cue,
            program.blob_ids,
            path.display()
        );
    }
    Ok(())
}

fn program_index_for_record(offset: u16) -> Result<usize, Box<dyn std::error::Error>> {
    oracle_audio::UPLOAD_PROGRAMS
        .iter()
        .position(|program| program.source_record_offset == offset)
        .ok_or_else(|| format!("resident audio record {offset:03X} is missing").into())
}

fn parse_selections(
    selection: Option<&str>,
) -> Result<Vec<ProgramSelection>, Box<dyn std::error::Error>> {
    let Some(selection) = selection else {
        return Ok((0..oracle_audio::SOURCE_MODE_COUNT)
            .map(ProgramSelection::SourceMode)
            .collect());
    };
    let selections = selection
        .split(',')
        .map(|value| {
            let value = value.trim();
            if value == "logo" {
                Ok(ProgramSelection::Semantic(SemanticCue::LogoPresentation))
            } else if value == "title" {
                Ok(ProgramSelection::Semantic(SemanticCue::FormationAndTitle))
            } else if value == "briefing" {
                Ok(ProgramSelection::Semantic(SemanticCue::AndrossBriefing))
            } else if value == "map" {
                Ok(ProgramSelection::Semantic(SemanticCue::StrategicMap))
            } else if value == "pilots" {
                Ok(ProgramSelection::Semantic(SemanticCue::PilotSelection))
            } else if value == "open-space" {
                Ok(ProgramSelection::Semantic(SemanticCue::OpenSpaceCombat))
            } else if value == "fighter-intercept" {
                Ok(ProgramSelection::Semantic(SemanticCue::FighterIntercept))
            } else if value == "titania" {
                Ok(ProgramSelection::Semantic(SemanticCue::TitaniaBase))
            } else if value == "eladard" {
                Ok(ProgramSelection::Semantic(SemanticCue::EladardBase))
            } else if value == "carrier" {
                Ok(ProgramSelection::Semantic(SemanticCue::BattleCarrier))
            } else if value == "mirage" {
                Ok(ProgramSelection::Semantic(SemanticCue::MirageDragon))
            } else if value == "rival" {
                Ok(ProgramSelection::Semantic(SemanticCue::RivalEncounter))
            } else if value == "astropolis" {
                Ok(ProgramSelection::Semantic(SemanticCue::AstropolisAssault))
            } else if value == "game-over" {
                Ok(ProgramSelection::Semantic(SemanticCue::GameOverAndContinue))
            } else if value == "ending" {
                Ok(ProgramSelection::Semantic(SemanticCue::CreditsAndEnding))
            } else if let Some(offset) = value.strip_prefix('r') {
                u16::from_str_radix(offset, 16)
                    .map(ProgramSelection::RecordOffset)
                    .map_err(|error| error.to_string())
            } else {
                value
                    .parse::<usize>()
                    .map(ProgramSelection::SourceMode)
                    .map_err(|error| error.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(invalid) = selections.iter().find_map(|selection| match selection {
        ProgramSelection::SourceMode(mode) if *mode >= oracle_audio::SOURCE_MODE_COUNT => {
            Some(mode)
        }
        _ => None,
    }) {
        return Err(format!("source mode {invalid} is outside the retail table").into());
    }
    Ok(selections)
}

fn boot_program(
    asset_dir: &Path,
    resident_program: Option<&oracle_audio::UploadProgram>,
    program: &oracle_audio::UploadProgram,
) -> Result<SpcPlayer, Box<dyn std::error::Error>> {
    let resident_blob_count = resident_program
        .map(|resident| resident.blob_ids.len())
        .unwrap_or_default();
    let mut files = Vec::with_capacity(
        1 + oracle_audio::BASE_BOOT_BLOB_IDS.len() + resident_blob_count + program.blob_ids.len(),
    );
    files.push(audio::DRIVER_UPLOAD_FILE);
    files.extend(
        oracle_audio::BASE_BOOT_BLOB_IDS
            .iter()
            .map(|blob| audio::AUDIO_BLOBS[usize::from(*blob)].file),
    );
    if let Some(resident) = resident_program {
        files.extend(
            resident
                .blob_ids
                .iter()
                .map(|blob| audio::AUDIO_BLOBS[usize::from(*blob)].file),
        );
    }
    files.extend(
        program
            .blob_ids
            .iter()
            .map(|blob| audio::AUDIO_BLOBS[usize::from(*blob)].file),
    );
    let player = SpcPlayer::new();
    player.load_files(&files, asset_dir, audio::SPC_DRIVER_ENTRY)?;
    Ok(player)
}

fn render(player: &SpcPlayer, seconds: usize) -> Vec<i16> {
    let total_samples = seconds * SAMPLE_RATE * 2;
    let mut output = Vec::with_capacity(total_samples);
    let mut buffer = vec![0i16; RENDER_BUFFER_FRAMES * 2];
    while output.len() < total_samples {
        player.generate(&mut buffer);
        let remaining = total_samples - output.len();
        output.extend_from_slice(&buffer[..remaining.min(buffer.len())]);
    }
    output
}

fn root_mean_square(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum = samples
        .iter()
        .map(|sample| {
            let value = i64::from(*sample);
            value * value
        })
        .sum::<i64>();
    (sum as f64 / samples.len() as f64).sqrt()
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
