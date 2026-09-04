//! Typed PCM mixer used by the shipping port.
//!
//! PCM assets are produced offline from the verification oracle. Runtime
//! playback has no source-machine program, memory image, or hardware ports.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::catalog;

const CHANNEL_COUNT: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const MAX_EFFECT_VOICES: usize = 16;
const UNITY_GAIN: i32 = 32_768;
/// Source-driver fade duration measured against the oracle: 65,536 output
/// frames, followed only by the filter tail.
const MUSIC_FADE_FRAMES: usize = 65_536;
/// Semantic acknowledgement values used by the source-compatible sound
/// queue when pausing and resuming the native mixer.
const PAUSE_ENABLED_ACKNOWLEDGEMENT: u8 = 2;
const PAUSE_DISABLED_ACKNOWLEDGEMENT: u8 = 1;

#[derive(Debug)]
pub enum NativeAudioError {
    MissingAsset(Vec<PathBuf>),
    Io(std::io::Error),
    InvalidWave(PathBuf, &'static str),
}

impl fmt::Display for NativeAudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NativeAudioError::MissingAsset(paths) => {
                write!(formatter, "native audio asset not found")?;
                for path in paths {
                    write!(formatter, " {}", path.display())?;
                }
                Ok(())
            }
            NativeAudioError::Io(error) => write!(formatter, "native audio I/O: {error}"),
            NativeAudioError::InvalidWave(path, reason) => {
                write!(
                    formatter,
                    "invalid native audio WAV {}: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for NativeAudioError {}

impl From<std::io::Error> for NativeAudioError {
    fn from(value: std::io::Error) -> Self {
        NativeAudioError::Io(value)
    }
}

#[derive(Debug)]
struct PcmClip {
    samples: Vec<i16>,
    loop_start: usize,
}

#[derive(Debug, Clone)]
struct Voice {
    clip: Arc<PcmClip>,
    position: usize,
    looping: bool,
}

impl Voice {
    fn new(clip: Arc<PcmClip>, looping: bool) -> Self {
        Self {
            clip,
            position: 0,
            looping,
        }
    }

    fn next_stereo(&mut self) -> Option<(i16, i16)> {
        if self.position + 1 >= self.clip.samples.len() {
            if !self.looping || self.clip.samples.is_empty() {
                return None;
            }
            self.position = self.clip.loop_start.min(self.clip.samples.len() / 2) * 2;
        }
        let left = self.clip.samples[self.position];
        let right = self.clip.samples[self.position + 1];
        self.position += 2;
        Some((left, right))
    }
}

struct MixerState {
    asset_root: PathBuf,
    loaded_track: Option<u8>,
    music: Option<Voice>,
    engine: Option<(u8, Voice)>,
    ambience: Option<(u8, Voice)>,
    positional: Option<(PathBuf, Voice)>,
    effects: Vec<Voice>,
    cache: HashMap<PathBuf, Arc<PcmClip>>,
    missing: HashSet<PathBuf>,
    last_effect_consumed: Option<u8>,
    paused: bool,
    music_gain: i32,
    music_fade_origin_gain: i32,
    music_target_gain: i32,
    music_fade_remaining: usize,
}

impl MixerState {
    fn load_candidates(
        &mut self,
        candidates: Vec<PathBuf>,
    ) -> Result<Arc<PcmClip>, NativeAudioError> {
        for path in &candidates {
            if let Some(clip) = self.cache.get(path) {
                return Ok(clip.clone());
            }
            if self.missing.contains(path) || !path.is_file() {
                self.missing.insert(path.clone());
                continue;
            }
            let clip = Arc::new(read_wave(path)?);
            self.cache.insert(path.clone(), clip.clone());
            return Ok(clip);
        }
        Err(NativeAudioError::MissingAsset(candidates))
    }

    fn music_paths(&self, cue: u8) -> Vec<PathBuf> {
        let track = self.loaded_track.unwrap_or_default();
        vec![self
            .asset_root
            .join("music")
            .join(format!("track_{track:02}_cue_{cue:02X}.wav"))]
    }

    fn effect_paths(&self, effect: u8) -> Vec<PathBuf> {
        let mut paths = Vec::with_capacity(2);
        if let Some(track) = self.loaded_track {
            paths.push(
                self.asset_root
                    .join("effects")
                    .join(format!("track_{track:02}_{effect:02X}.wav")),
            );
        }
        paths.push(
            self.asset_root
                .join("effects")
                .join(format!("{effect:02X}.wav")),
        );
        paths
    }

    fn loop_path(&self, channel: &str, sound: u8) -> PathBuf {
        self.asset_root
            .join(channel)
            .join(format!("{sound:02X}.wav"))
    }
}

/// Thread-safe handle shared by the game thread and SDL audio callback.
#[derive(Clone)]
pub struct NativePlayer {
    inner: Arc<Mutex<MixerState>>,
}

impl NativePlayer {
    pub fn new(asset_dir: impl AsRef<Path>) -> Self {
        Self::with_asset_root(asset_dir.as_ref().join("native_audio"))
    }

    /// Construct a mixer for a game-specific native PCM catalog.
    pub fn with_asset_root(asset_root: impl AsRef<Path>) -> Self {
        let player = Self {
            inner: Arc::new(Mutex::new(MixerState {
                asset_root: asset_root.as_ref().to_path_buf(),
                loaded_track: None,
                music: None,
                engine: None,
                ambience: None,
                positional: None,
                effects: Vec::new(),
                cache: HashMap::new(),
                missing: HashSet::new(),
                last_effect_consumed: None,
                paused: false,
                music_gain: UNITY_GAIN,
                music_fade_origin_gain: UNITY_GAIN,
                music_target_gain: UNITY_GAIN,
                music_fade_remaining: 0,
            })),
        };
        // Decode the finite one-shot effect catalog before the SDL callback
        // can contend with the game thread. Music remains lazy because the
        // shipped music catalog is large. Runtime commands retain their
        // normal missing/invalid-asset errors: preload only caches files that
        // decode successfully, and deliberately ignores all other entries.
        player.preload_available_effects();
        player
    }

    fn preload_available_effects(&self) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        let path = state.asset_root.join("effects");
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("wav") {
                continue;
            }
            // Do not record failures in `missing`: a later command must
            // still report InvalidWave/IO with the same source path.
            if let Ok(clip) = read_wave(&path) {
                state.cache.insert(path, Arc::new(clip));
            }
        }
    }

    pub fn load_track(&self, track: u8) {
        let mut state = self.inner.lock().unwrap();
        state.loaded_track = Some(track);
        state.music = None;
        state.engine = None;
        state.ambience = None;
        state.positional = None;
        state.effects.clear();
        state.last_effect_consumed = None;
        state.music_gain = UNITY_GAIN;
        state.music_fade_origin_gain = UNITY_GAIN;
        state.music_target_gain = UNITY_GAIN;
        state.music_fade_remaining = 0;
    }

    /// Check the complete SF1 PCM catalog before entering the game loop.
    ///
    /// Runtime playback never falls back to the source-machine sound program;
    /// a missing certified asset is therefore a startup error, not silence.
    pub fn validate_star_fox_assets(&self) -> Result<(), NativeAudioError> {
        let state = self.inner.lock().unwrap();
        let mut required = Vec::new();

        for track in 0..catalog::SND_TRACK_COUNT {
            let cue = catalog::track_start_cue(track);
            required.push(
                state
                    .asset_root
                    .join("music")
                    .join(format!("track_{track:02}_cue_{cue:02X}.wav")),
            );
        }
        for track in catalog::GAMEPLAY_TRACKS {
            for cue in catalog::GAMEPLAY_MUSIC_CUES {
                required.push(
                    state
                        .asset_root
                        .join("music")
                        .join(format!("track_{track:02}_cue_{cue:02X}.wav")),
                );
            }
        }
        for cue in catalog::PLANET_SELECTION_MUSIC_CUES {
            required.push(
                state
                    .asset_root
                    .join("music")
                    .join(format!("track_{:02}_cue_{cue:02X}.wav", catalog::SND_MAP)),
            );
        }
        required.extend([
            state.asset_root.join("music/track_07_cue_F4.wav"),
            state.asset_root.join("music/track_07_cue_F5.wav"),
            state.asset_root.join("music/track_15_cue_12.wav"),
            state.asset_root.join("music/track_15_cue_13.wav"),
        ]);
        for sound in 1..=u8::MAX {
            let name = format!("{sound:02X}.wav");
            required.push(state.asset_root.join("effects").join(&name));
            required.push(state.asset_root.join("engine").join(&name));
            required.push(state.asset_root.join("ambience").join(name));
        }

        required.sort_unstable();
        required.dedup();
        let missing = required
            .into_iter()
            .filter(|path| !path.is_file())
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(NativeAudioError::MissingAsset(missing))
        }
    }

    pub fn validate_named_music(&self, files: &[&str]) -> Result<(), NativeAudioError> {
        self.validate_named_files("music", files)
    }

    pub fn validate_named_effects(&self, files: &[&str]) -> Result<(), NativeAudioError> {
        self.validate_named_files("effects", files)
    }

    pub fn validate_named_engine(&self, files: &[&str]) -> Result<(), NativeAudioError> {
        self.validate_named_files("engine", files)
    }

    pub fn validate_named_ambience(&self, files: &[&str]) -> Result<(), NativeAudioError> {
        self.validate_named_files("ambience", files)
    }

    pub fn validate_named_positional(&self, files: &[&str]) -> Result<(), NativeAudioError> {
        self.validate_named_files("positional", files)
    }

    fn validate_named_files(
        &self,
        directory: &str,
        files: &[&str],
    ) -> Result<(), NativeAudioError> {
        let state = self.inner.lock().unwrap();
        let missing = files
            .iter()
            .map(|file| state.asset_root.join(directory).join(file))
            .filter(|path| !path.is_file())
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(NativeAudioError::MissingAsset(missing))
        }
    }

    pub fn start_music(&self, cue: u8) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        match cue {
            catalog::MUSIC_STOP => {
                state.music = None;
                return Ok(());
            }
            catalog::MUSIC_FADE_OUT => {
                state.music_fade_origin_gain = state.music_gain;
                state.music_target_gain = 0;
                state.music_fade_remaining = MUSIC_FADE_FRAMES;
                return Ok(());
            }
            _ => {}
        }
        let paths = state.music_paths(cue);
        let clip = state.load_candidates(paths)?;
        state.music = Some(Voice::new(clip, true));
        state.music_gain = UNITY_GAIN;
        state.music_fade_origin_gain = UNITY_GAIN;
        state.music_target_gain = UNITY_GAIN;
        state.music_fade_remaining = 0;
        Ok(())
    }

    pub fn start_named_music(&self, file: &str) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        let path = state.asset_root.join("music").join(file);
        let clip = state.load_candidates(vec![path])?;
        state.music = Some(Voice::new(clip, true));
        state.music_gain = UNITY_GAIN;
        state.music_fade_origin_gain = UNITY_GAIN;
        state.music_target_gain = UNITY_GAIN;
        state.music_fade_remaining = 0;
        Ok(())
    }

    pub fn play_effect(&self, effect: u8) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        state.last_effect_consumed = Some(effect);
        let paths = state.effect_paths(effect);
        let clip = state.load_candidates(paths)?;
        if state.effects.len() == MAX_EFFECT_VOICES {
            state.effects.remove(0);
        }
        state.effects.push(Voice::new(clip, false));
        Ok(())
    }

    pub fn play_named_effect(&self, file: &str) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        let path = state.asset_root.join("effects").join(file);
        let clip = state.load_candidates(vec![path])?;
        if state.effects.len() == MAX_EFFECT_VOICES {
            state.effects.remove(0);
        }
        state.effects.push(Voice::new(clip, false));
        Ok(())
    }

    pub fn effect_consumed(&self, effect: u8) -> bool {
        self.inner.lock().unwrap().last_effect_consumed == Some(effect)
    }

    pub fn clear_effect_acknowledgement(&self) {
        self.inner.lock().unwrap().last_effect_consumed = None;
    }

    pub fn set_engine_sound(&self, sound: u8) -> Result<(), NativeAudioError> {
        self.set_looping_channel(sound, "engine", true)
    }

    pub fn set_named_engine(&self, file: Option<&str>) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        state.engine = None;
        let Some(file) = file else {
            return Ok(());
        };
        let path = state.asset_root.join("engine").join(file);
        let clip = state.load_candidates(vec![path])?;
        state.engine = Some((0, Voice::new(clip, true)));
        Ok(())
    }

    pub fn set_named_ambience(
        &self,
        file: Option<&str>,
        looping: bool,
    ) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        state.ambience = None;
        let Some(file) = file else {
            return Ok(());
        };
        let path = state.asset_root.join("ambience").join(file);
        let clip = state.load_candidates(vec![path])?;
        state.ambience = Some((0, Voice::new(clip, looping)));
        Ok(())
    }

    pub fn set_named_positional(
        &self,
        file: Option<&str>,
        restart: bool,
    ) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        let Some(file) = file else {
            state.positional = None;
            return Ok(());
        };
        let path = state.asset_root.join("positional").join(file);
        if !restart
            && state
                .positional
                .as_ref()
                .is_some_and(|(active, _)| *active == path)
        {
            return Ok(());
        }
        let clip = state.load_candidates(vec![path.clone()])?;
        state.positional = Some((path, Voice::new(clip, true)));
        Ok(())
    }

    pub fn set_ambient_sound(&self, sound: u8) -> Result<(), NativeAudioError> {
        self.set_looping_channel(sound, "ambience", false)
    }

    fn set_looping_channel(
        &self,
        sound: u8,
        channel: &'static str,
        engine: bool,
    ) -> Result<(), NativeAudioError> {
        let mut state = self.inner.lock().unwrap();
        let current = if engine {
            &state.engine
        } else {
            &state.ambience
        };
        if current.as_ref().is_some_and(|(active, _)| *active == sound) {
            return Ok(());
        }
        if engine {
            state.engine = None;
        } else {
            state.ambience = None;
        }
        if sound == 0 {
            return Ok(());
        }
        let path = state.loop_path(channel, sound);
        let clip = state.load_candidates(vec![path])?;
        let voice = Some((sound, Voice::new(clip, true)));
        if engine {
            state.engine = voice;
        } else {
            state.ambience = voice;
        }
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) {
        let mut state = self.inner.lock().unwrap();
        state.paused = paused;
        // The source sound driver echoes the pause command immediately. The
        // native mixer performs the state change synchronously, so publish
        // the equivalent acknowledgement here; otherwise the sound queue
        // waits forever on the pause-on command and never applies resume.
        state.last_effect_consumed = Some(if paused {
            PAUSE_ENABLED_ACKNOWLEDGEMENT
        } else {
            PAUSE_DISABLED_ACKNOWLEDGEMENT
        });
    }

    /// Fill an interleaved 32 kHz stereo buffer.
    pub fn generate(&self, output: &mut [i16]) {
        assert_eq!(output.len() % 2, 0, "stereo sample pairs");
        let mut state = self.inner.lock().unwrap();
        if state.paused {
            output.fill(0);
            return;
        }

        for frame in output.chunks_exact_mut(2) {
            if state.music_fade_remaining > 0 {
                let elapsed = MUSIC_FADE_FRAMES - state.music_fade_remaining + 1;
                let fade_range = state.music_target_gain - state.music_fade_origin_gain;
                state.music_gain = state.music_fade_origin_gain
                    + fade_range * elapsed as i32 / MUSIC_FADE_FRAMES as i32;
                state.music_fade_remaining -= 1;
                if state.music_fade_remaining == 0 {
                    state.music_gain = state.music_target_gain;
                }
            }

            let mut left = 0i32;
            let mut right = 0i32;
            let music_gain = state.music_gain;
            if let Some(music) = &mut state.music {
                if let Some((sample_left, sample_right)) = music.next_stereo() {
                    left += i32::from(sample_left) * music_gain / UNITY_GAIN;
                    right += i32::from(sample_right) * music_gain / UNITY_GAIN;
                }
            }
            if let Some((_, voice)) = &mut state.engine {
                if let Some((sample_left, sample_right)) = voice.next_stereo() {
                    left += i32::from(sample_left);
                    right += i32::from(sample_right);
                }
            }
            if let Some((_, voice)) = &mut state.ambience {
                if let Some((sample_left, sample_right)) = voice.next_stereo() {
                    left += i32::from(sample_left);
                    right += i32::from(sample_right);
                }
            }
            if let Some((_, voice)) = &mut state.positional {
                if let Some((sample_left, sample_right)) = voice.next_stereo() {
                    left += i32::from(sample_left);
                    right += i32::from(sample_right);
                }
            }
            state.effects.retain_mut(|voice| {
                if let Some((sample_left, sample_right)) = voice.next_stereo() {
                    left += i32::from(sample_left);
                    right += i32::from(sample_right);
                    true
                } else {
                    false
                }
            });
            frame[0] = left.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            frame[1] = right.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
    }
}

fn read_wave(path: &Path) -> Result<PcmClip, NativeAudioError> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(NativeAudioError::InvalidWave(
            path.to_path_buf(),
            "missing RIFF/WAVE header",
        ));
    }

    let mut format = None;
    let mut sample_data = None;
    let mut cursor = 12usize;
    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_len =
            u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        cursor += 8;
        let end = cursor
            .checked_add(chunk_len)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                NativeAudioError::InvalidWave(path.to_path_buf(), "chunk extends past file")
            })?;
        if chunk_id == b"fmt " && chunk_len >= 16 {
            let encoding = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
            let channels = u16::from_le_bytes(bytes[cursor + 2..cursor + 4].try_into().unwrap());
            let bits = u16::from_le_bytes(bytes[cursor + 14..cursor + 16].try_into().unwrap());
            format = Some((encoding, channels, bits));
        } else if chunk_id == b"data" {
            sample_data = Some(&bytes[cursor..end]);
        }
        cursor = end + (chunk_len & 1);
    }

    if format != Some((1, CHANNEL_COUNT, BITS_PER_SAMPLE)) {
        return Err(NativeAudioError::InvalidWave(
            path.to_path_buf(),
            "expected 16-bit stereo PCM",
        ));
    }
    let sample_data = sample_data
        .ok_or_else(|| NativeAudioError::InvalidWave(path.to_path_buf(), "missing data chunk"))?;
    if sample_data.len() % 4 != 0 {
        return Err(NativeAudioError::InvalidWave(
            path.to_path_buf(),
            "partial stereo frame",
        ));
    }
    let samples = sample_data
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    let loop_path = path.with_extension("loop");
    let loop_start = std::fs::read_to_string(loop_path)
        .ok()
        .and_then(|text| text.trim().parse::<usize>().ok())
        .unwrap_or(0);
    Ok(PcmClip {
        samples,
        loop_start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TRACK: u8 = 2;
    const TEST_SAMPLE: i16 = 1_000;
    const TEST_FRAME_COUNT: usize = 4;

    fn temporary_asset_dir(test: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sf-audio-native-player-{}-{test}",
            std::process::id()
        ))
    }

    fn write_constant_wave(path: &Path, sample: i16) {
        const HEADER_LENGTH: usize = 44;
        const FORMAT_CHUNK_LENGTH: u32 = 16;
        const PCM_ENCODING: u16 = 1;
        const BYTES_PER_FRAME: u16 = 4;
        const SAMPLE_RATE: u32 = 32_000;

        let sample_count = TEST_FRAME_COUNT * usize::from(CHANNEL_COUNT);
        let data_length = u32::try_from(sample_count * 2).unwrap();
        let mut bytes = Vec::with_capacity(HEADER_LENGTH + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&FORMAT_CHUNK_LENGTH.to_le_bytes());
        bytes.extend_from_slice(&PCM_ENCODING.to_le_bytes());
        bytes.extend_from_slice(&CHANNEL_COUNT.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&(SAMPLE_RATE * u32::from(BYTES_PER_FRAME)).to_le_bytes());
        bytes.extend_from_slice(&BYTES_PER_FRAME.to_le_bytes());
        bytes.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for _ in 0..sample_count {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn missing_assets_are_reported() {
        let player = NativePlayer::new("/definitely/not/starfox-data");
        player.load_track(2);
        assert!(matches!(
            player.start_music(18),
            Err(NativeAudioError::MissingAsset(_))
        ));
    }

    #[test]
    fn available_effect_is_decoded_before_runtime_commands() {
        const TEST_EFFECT: u8 = 0x35;
        let asset_dir = temporary_asset_dir("preload");
        let native_dir = asset_dir.join("native_audio");
        let effects_dir = native_dir.join("effects");
        if asset_dir.exists() {
            std::fs::remove_dir_all(&asset_dir).unwrap();
        }
        std::fs::create_dir_all(&effects_dir).unwrap();
        let effect_path = effects_dir.join(format!("{TEST_EFFECT:02X}.wav"));
        write_constant_wave(&effect_path, TEST_SAMPLE);

        let player = NativePlayer::new(&asset_dir);
        let state = player.inner.lock().unwrap();
        assert!(state.cache.contains_key(&effect_path));
        assert!(state.cache.keys().all(|path| path.starts_with(&effects_dir)));
        drop(state);

        // The runtime command must use the decoded clip, not reopen the file.
        std::fs::remove_file(&effect_path).unwrap();
        player.play_effect(TEST_EFFECT).unwrap();
        let mut output = [0i16; 2];
        player.generate(&mut output);
        assert_eq!(output, [TEST_SAMPLE, TEST_SAMPLE]);
        std::fs::remove_dir_all(asset_dir).unwrap();
    }

    #[test]
    fn planet_selection_assets_are_required_at_startup() {
        let asset_root = temporary_asset_dir("planet-selection-assets");
        let player = NativePlayer::with_asset_root(&asset_root);
        let error = player
            .validate_star_fox_assets()
            .expect_err("an empty catalog must fail validation");
        let NativeAudioError::MissingAsset(paths) = error else {
            panic!("empty catalog returned the wrong error type");
        };

        for cue in catalog::PLANET_SELECTION_MUSIC_CUES {
            let expected = asset_root
                .join("music")
                .join(format!("track_05_cue_{cue:02X}.wav"));
            assert!(
                paths.contains(&expected),
                "missing required {}",
                expected.display()
            );
        }
    }

    #[test]
    fn pause_state_acknowledges_both_transitions() {
        let player = NativePlayer::with_asset_root(temporary_asset_dir("pause-ack"));

        player.set_paused(true);
        assert!(player.effect_consumed(PAUSE_ENABLED_ACKNOWLEDGEMENT));
        let mut muted = [TEST_SAMPLE; 2];
        player.generate(&mut muted);
        assert_eq!(muted, [0, 0]);

        player.set_paused(false);
        assert!(player.effect_consumed(PAUSE_DISABLED_ACKNOWLEDGEMENT));
    }

    #[test]
    fn all_clear_is_music_and_fade_out_matches_oracle_duration() {
        let asset_dir = temporary_asset_dir("music-fade");
        let music_dir = asset_dir.join("native_audio/music");
        if asset_dir.exists() {
            std::fs::remove_dir_all(&asset_dir).unwrap();
        }
        std::fs::create_dir_all(&music_dir).unwrap();
        write_constant_wave(&music_dir.join("track_02_cue_F0.wav"), TEST_SAMPLE);

        let player = NativePlayer::new(&asset_dir);
        player.load_track(TEST_TRACK);
        player.start_music(catalog::MUSIC_ALL_CLEAR).unwrap();
        let mut initial = [0i16; 2];
        player.generate(&mut initial);
        assert_eq!(initial, [TEST_SAMPLE, TEST_SAMPLE]);

        player.start_music(catalog::MUSIC_FADE_OUT).unwrap();
        let mut fade = vec![0i16; MUSIC_FADE_FRAMES * 2];
        player.generate(&mut fade);
        let midpoint = MUSIC_FADE_FRAMES;
        assert!((490..=510).contains(&fade[midpoint]));
        assert_eq!(&fade[fade.len() - 2..], &[0, 0]);

        std::fs::remove_dir_all(asset_dir).unwrap();
    }

    #[test]
    fn named_ambience_is_nonlooping_and_replaced_atomically() {
        const REPLACEMENT_SAMPLE: i16 = -2_000;

        let asset_root = temporary_asset_dir("named-ambience");
        let ambience_dir = asset_root.join("ambience/open_space/fox");
        if asset_root.exists() {
            std::fs::remove_dir_all(&asset_root).unwrap();
        }
        std::fs::create_dir_all(&ambience_dir).unwrap();
        write_constant_wave(&ambience_dir.join("building.wav"), TEST_SAMPLE);
        write_constant_wave(&ambience_dir.join("ready.wav"), REPLACEMENT_SAMPLE);

        let player = NativePlayer::with_asset_root(&asset_root);
        player
            .validate_named_ambience(&["open_space/fox/building.wav", "open_space/fox/ready.wav"])
            .unwrap();

        player
            .set_named_ambience(Some("open_space/fox/building.wav"), false)
            .unwrap();
        let mut finite_output = [0i16; (TEST_FRAME_COUNT + 1) * 2];
        player.generate(&mut finite_output);
        assert_eq!(
            &finite_output[..TEST_FRAME_COUNT * 2],
            &[TEST_SAMPLE; TEST_FRAME_COUNT * 2]
        );
        assert_eq!(&finite_output[TEST_FRAME_COUNT * 2..], &[0, 0]);

        player
            .set_named_ambience(Some("open_space/fox/building.wav"), false)
            .unwrap();
        let mut started = [0i16; 2];
        player.generate(&mut started);
        assert_eq!(started, [TEST_SAMPLE, TEST_SAMPLE]);
        player
            .set_named_ambience(Some("open_space/fox/ready.wav"), false)
            .unwrap();
        let mut replaced = [0i16; 2];
        player.generate(&mut replaced);
        assert_eq!(replaced, [REPLACEMENT_SAMPLE, REPLACEMENT_SAMPLE]);

        player.set_named_ambience(None, false).unwrap();
        let mut stopped = [TEST_SAMPLE; 2];
        player.generate(&mut stopped);
        assert_eq!(stopped, [0, 0]);

        std::fs::remove_dir_all(asset_root).unwrap();
    }

    #[test]
    fn positional_loop_mixes_independently_from_charge_ambience() {
        const POSITIONAL_SAMPLE: i16 = 2_000;

        let asset_root = temporary_asset_dir("positional-loop");
        let ambience_dir = asset_root.join("ambience/open_space/fox");
        let positional_dir = asset_root.join("positional/open_space/fox");
        if asset_root.exists() {
            std::fs::remove_dir_all(&asset_root).unwrap();
        }
        std::fs::create_dir_all(&ambience_dir).unwrap();
        std::fs::create_dir_all(&positional_dir).unwrap();
        write_constant_wave(&ambience_dir.join("charge.wav"), TEST_SAMPLE);
        write_constant_wave(&positional_dir.join("capital.wav"), POSITIONAL_SAMPLE);

        let player = NativePlayer::with_asset_root(&asset_root);
        player
            .validate_named_positional(&["open_space/fox/capital.wav"])
            .unwrap();
        player
            .set_named_ambience(Some("open_space/fox/charge.wav"), false)
            .unwrap();
        player
            .set_named_positional(Some("open_space/fox/capital.wav"), false)
            .unwrap();

        let mut combined = [0i16; 2];
        player.generate(&mut combined);
        assert_eq!(combined, [TEST_SAMPLE + POSITIONAL_SAMPLE; 2]);

        let position_after_mix = player
            .inner
            .lock()
            .unwrap()
            .positional
            .as_ref()
            .unwrap()
            .1
            .position;
        player
            .set_named_positional(Some("open_space/fox/capital.wav"), false)
            .unwrap();
        assert_eq!(
            player
                .inner
                .lock()
                .unwrap()
                .positional
                .as_ref()
                .unwrap()
                .1
                .position,
            position_after_mix
        );
        player
            .set_named_positional(Some("open_space/fox/capital.wav"), true)
            .unwrap();
        assert_eq!(
            player
                .inner
                .lock()
                .unwrap()
                .positional
                .as_ref()
                .unwrap()
                .1
                .position,
            0
        );

        player.set_named_ambience(None, false).unwrap();
        let mut positional_only = [0i16; 2];
        player.generate(&mut positional_only);
        assert_eq!(positional_only, [POSITIONAL_SAMPLE; 2]);

        player.set_named_positional(None, false).unwrap();
        let mut stopped = [TEST_SAMPLE; 2];
        player.generate(&mut stopped);
        assert_eq!(stopped, [0, 0]);

        std::fs::remove_dir_all(asset_root).unwrap();
    }
}
