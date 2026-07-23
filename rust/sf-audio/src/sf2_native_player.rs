//! Typed native PCM surface for Star Fox 2.
//!
//! The original sound program is used only by the offline renderer. Runtime
//! selects semantic cues and mixes ordinary PCM assets.

use std::path::Path;

use crate::native_player::{NativeAudioError, NativePlayer};

const SF2_AUDIO_DIRECTORY: &str = "native_audio_sf2";
const LOGO_PRESENTATION_FILE: &str = "logo_presentation.wav";
const FORMATION_AND_TITLE_FILE: &str = "formation_and_title.wav";
const ANDROSS_BRIEFING_FILE: &str = "andross_briefing.wav";
const STRATEGIC_MAP_FILE: &str = "strategic_map.wav";
const PILOT_SELECTION_FILE: &str = "pilot_selection.wav";
const OPEN_SPACE_COMBAT_FILE: &str = "open_space_combat.wav";
const FIGHTER_INTERCEPT_FILE: &str = "fighter_intercept.wav";
const TITANIA_BASE_FILE: &str = "titania_base.wav";
const ELADARD_BASE_FILE: &str = "eladard_base.wav";
const BATTLE_CARRIER_FILE: &str = "battle_carrier.wav";
const MIRAGE_DRAGON_FILE: &str = "mirage_dragon.wav";
const RIVAL_ENCOUNTER_FILE: &str = "rival_encounter.wav";
const ASTROPOLIS_ASSAULT_FILE: &str = "astropolis_assault.wav";
const GAME_OVER_AND_CONTINUE_FILE: &str = "game_over_and_continue.wav";
const CREDITS_AND_ENDING_FILE: &str = "credits_and_ending.wav";
const RAPID_LASER_FILE: &str = "rapid_laser.wav";
const CHARGE_BUILDING_FILE: &str = "charge_building.wav";
const CHARGE_READY_FILE: &str = "charge_ready.wav";
const CHARGED_LASER_FILE: &str = "charged_laser.wav";
const HOSTILE_LASER_FILE: &str = "hostile_laser.wav";
const FLIGHT_ENGINE_FILE: &str = "flight.wav";
const MUSIC_CUE_COUNT: usize = 15;
const SOUND_EFFECT_COUNT: usize = 3;
const ENGINE_CUE_COUNT: usize = 1;
const CHARGE_CUE_COUNT: usize = 2;
const SOUND_BANK_COUNT: usize = 8;
const SOUND_PILOT_COUNT: usize = 6;
const REQUIRED_MUSIC: [&str; MUSIC_CUE_COUNT] = [
    LOGO_PRESENTATION_FILE,
    FORMATION_AND_TITLE_FILE,
    ANDROSS_BRIEFING_FILE,
    STRATEGIC_MAP_FILE,
    PILOT_SELECTION_FILE,
    OPEN_SPACE_COMBAT_FILE,
    FIGHTER_INTERCEPT_FILE,
    TITANIA_BASE_FILE,
    ELADARD_BASE_FILE,
    BATTLE_CARRIER_FILE,
    MIRAGE_DRAGON_FILE,
    RIVAL_ENCOUNTER_FILE,
    ASTROPOLIS_ASSAULT_FILE,
    GAME_OVER_AND_CONTINUE_FILE,
    CREDITS_AND_ENDING_FILE,
];
const REQUIRED_EFFECTS: [&str; SOUND_EFFECT_COUNT] = [
    RAPID_LASER_FILE,
    CHARGED_LASER_FILE,
    HOSTILE_LASER_FILE,
];
const REQUIRED_ENGINE: [&str; ENGINE_CUE_COUNT] = [FLIGHT_ENGINE_FILE];
const REQUIRED_AMBIENCE: [&str; CHARGE_CUE_COUNT] = [CHARGE_BUILDING_FILE, CHARGE_READY_FILE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MusicCue {
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

impl Sf2MusicCue {
    const fn filename(self) -> &'static str {
        match self {
            Self::LogoPresentation => LOGO_PRESENTATION_FILE,
            Self::FormationAndTitle => FORMATION_AND_TITLE_FILE,
            Self::AndrossBriefing => ANDROSS_BRIEFING_FILE,
            Self::StrategicMap => STRATEGIC_MAP_FILE,
            Self::PilotSelection => PILOT_SELECTION_FILE,
            Self::OpenSpaceCombat => OPEN_SPACE_COMBAT_FILE,
            Self::FighterIntercept => FIGHTER_INTERCEPT_FILE,
            Self::TitaniaBase => TITANIA_BASE_FILE,
            Self::EladardBase => ELADARD_BASE_FILE,
            Self::BattleCarrier => BATTLE_CARRIER_FILE,
            Self::MirageDragon => MIRAGE_DRAGON_FILE,
            Self::RivalEncounter => RIVAL_ENCOUNTER_FILE,
            Self::AstropolisAssault => ASTROPOLIS_ASSAULT_FILE,
            Self::GameOverAndContinue => GAME_OVER_AND_CONTINUE_FILE,
            Self::CreditsAndEnding => CREDITS_AND_ENDING_FILE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2SoundBank {
    OpenSpaceCombat,
    FighterIntercept,
    TitaniaBase,
    EladardBase,
    BattleCarrier,
    MirageDragon,
    RivalEncounter,
    AstropolisAssault,
}

impl Sf2SoundBank {
    const ALL: [Self; SOUND_BANK_COUNT] = [
        Self::OpenSpaceCombat,
        Self::FighterIntercept,
        Self::TitaniaBase,
        Self::EladardBase,
        Self::BattleCarrier,
        Self::MirageDragon,
        Self::RivalEncounter,
        Self::AstropolisAssault,
    ];

    const fn directory(self) -> &'static str {
        match self {
            Self::OpenSpaceCombat => "open_space",
            Self::FighterIntercept => "fighter_intercept",
            Self::TitaniaBase => "titania",
            Self::EladardBase => "eladard",
            Self::BattleCarrier => "carrier",
            Self::MirageDragon => "mirage",
            Self::RivalEncounter => "rival",
            Self::AstropolisAssault => "astropolis",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2SoundPilot {
    Fox,
    Falco,
    Peppy,
    Slippy,
    Miyu,
    Fay,
}

impl Sf2SoundPilot {
    const ALL: [Self; SOUND_PILOT_COUNT] = [
        Self::Fox,
        Self::Falco,
        Self::Peppy,
        Self::Slippy,
        Self::Miyu,
        Self::Fay,
    ];

    const fn directory(self) -> &'static str {
        match self {
            Self::Fox => "fox",
            Self::Falco => "falco",
            Self::Peppy => "peppy",
            Self::Slippy => "slippy",
            Self::Miyu => "miyu",
            Self::Fay => "fay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2SoundEffect {
    RapidLaser,
    ChargedLaser,
    HostileLaser,
}

impl Sf2SoundEffect {
    const fn filename(self) -> &'static str {
        match self {
            Self::RapidLaser => RAPID_LASER_FILE,
            Self::ChargedLaser => CHARGED_LASER_FILE,
            Self::HostileLaser => HOSTILE_LASER_FILE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2ChargeCue {
    Silent,
    Building,
    Ready,
}

impl Sf2ChargeCue {
    const fn filename(self) -> Option<&'static str> {
        match self {
            Self::Silent => None,
            Self::Building => Some(CHARGE_BUILDING_FILE),
            Self::Ready => Some(CHARGE_READY_FILE),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2EngineCue {
    Silent,
    Flight,
}

impl Sf2EngineCue {
    const fn filename(self) -> Option<&'static str> {
        match self {
            Self::Silent => None,
            Self::Flight => Some(FLIGHT_ENGINE_FILE),
        }
    }
}

#[derive(Clone)]
pub struct Sf2NativePlayer {
    mixer: NativePlayer,
}

impl Sf2NativePlayer {
    pub fn new(asset_dir: impl AsRef<Path>) -> Self {
        Self {
            mixer: NativePlayer::with_asset_root(asset_dir.as_ref().join(SF2_AUDIO_DIRECTORY)),
        }
    }

    pub fn validate_assets(&self) -> Result<(), NativeAudioError> {
        self.mixer.validate_named_music(&REQUIRED_MUSIC)?;
        let effects = Self::variant_files(&REQUIRED_EFFECTS);
        let engines = Self::variant_files(&REQUIRED_ENGINE);
        let ambience = Self::variant_files(&REQUIRED_AMBIENCE);
        self.mixer.validate_named_effects(&Self::file_refs(&effects))?;
        self.mixer.validate_named_engine(&Self::file_refs(&engines))?;
        self.mixer.validate_named_ambience(&Self::file_refs(&ambience))
    }

    pub fn start_music(&self, cue: Sf2MusicCue) -> Result<(), NativeAudioError> {
        self.mixer.start_named_music(cue.filename())
    }

    pub fn play_effect(
        &self,
        bank: Sf2SoundBank,
        pilot: Sf2SoundPilot,
        effect: Sf2SoundEffect,
    ) -> Result<(), NativeAudioError> {
        self.mixer
            .play_named_effect(&Self::variant_file(bank, pilot, effect.filename()))
    }

    pub fn set_engine(
        &self,
        bank: Sf2SoundBank,
        pilot: Sf2SoundPilot,
        cue: Sf2EngineCue,
    ) -> Result<(), NativeAudioError> {
        let file = cue
            .filename()
            .map(|file| Self::variant_file(bank, pilot, file));
        self.mixer.set_named_engine(file.as_deref())
    }

    pub fn set_charge(
        &self,
        bank: Sf2SoundBank,
        pilot: Sf2SoundPilot,
        cue: Sf2ChargeCue,
    ) -> Result<(), NativeAudioError> {
        let file = cue
            .filename()
            .map(|file| Self::variant_file(bank, pilot, file));
        self.mixer.set_named_ambience(file.as_deref(), false)
    }

    pub fn generate(&self, output: &mut [i16]) {
        self.mixer.generate(output);
    }

    fn variant_file(bank: Sf2SoundBank, pilot: Sf2SoundPilot, file: &str) -> String {
        format!("{}/{}/{}", bank.directory(), pilot.directory(), file)
    }

    fn variant_files(files: &[&str]) -> Vec<String> {
        let mut variants = Vec::with_capacity(
            Sf2SoundBank::ALL.len() * Sf2SoundPilot::ALL.len() * files.len(),
        );
        for bank in Sf2SoundBank::ALL {
            for pilot in Sf2SoundPilot::ALL {
                for file in files {
                    variants.push(Self::variant_file(bank, pilot, file));
                }
            }
        }
        variants
    }

    fn file_refs(files: &[String]) -> Vec<&str> {
        files.iter().map(String::as_str).collect()
    }
}
