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
const GAME_OVER_AND_CONTINUE_FILE: &str = "game_over_and_continue.wav";
const REQUIRED_MUSIC: [&str; 5] = [
    LOGO_PRESENTATION_FILE,
    FORMATION_AND_TITLE_FILE,
    ANDROSS_BRIEFING_FILE,
    STRATEGIC_MAP_FILE,
    GAME_OVER_AND_CONTINUE_FILE,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MusicCue {
    LogoPresentation,
    FormationAndTitle,
    AndrossBriefing,
    StrategicMap,
    GameOverAndContinue,
}

impl Sf2MusicCue {
    const fn filename(self) -> &'static str {
        match self {
            Self::LogoPresentation => LOGO_PRESENTATION_FILE,
            Self::FormationAndTitle => FORMATION_AND_TITLE_FILE,
            Self::AndrossBriefing => ANDROSS_BRIEFING_FILE,
            Self::StrategicMap => STRATEGIC_MAP_FILE,
            Self::GameOverAndContinue => GAME_OVER_AND_CONTINUE_FILE,
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
        self.mixer.validate_named_music(&REQUIRED_MUSIC)
    }

    pub fn start_music(&self, cue: Sf2MusicCue) -> Result<(), NativeAudioError> {
        self.mixer.start_named_music(cue.filename())
    }

    pub fn generate(&self, output: &mut [i16]) {
        self.mixer.generate(output);
    }
}
