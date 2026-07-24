//! Audio bridge: SDL3 playback stream driving the native PCM mixer, plus the
//! game-side Sound layer wired to shell state each tick.
//!
//! Port (C oracle): `src/audio/audio.c` (SDL audio device + callback) and the
//! `src/game/sound.c` call sites in
//! `Nmi_GameTick`/boot.c (here: `AudioSys::tick`). Certified assets use the
//! original 32000 Hz stereo rate; SDL3 resamples to the device rate.

use std::collections::HashSet;
use std::path::PathBuf;

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};
use sf_audio::native_player::{NativeAudioError, NativePlayer};
use sf_audio::sf2_native_player::{
    Sf2ChargeCue, Sf2EngineCue, Sf2MusicCue, Sf2NativePlayer, Sf2SoundBank, Sf2SoundEffect,
    Sf2SoundPilot, Sf2SpatialCue, Sf2SpatialDistance, Sf2StereoPosition,
};
use sf_audio::sound::{PosSndFamily, Sound, SoundBackend, SoundGameState, SoundObj, SoundPlayer};
use sf_game::game::{Game, PosSndFamilyId};
use sf_game::shell::{FrameSnapshot, GameState, Shell, SoundCmd};

/// Native asset output rate.
const AUDIO_RATE: i32 = 32000;

/// C `ASF4_DONESND` (src/game/obj.h:116) — write-back flag from the sound
/// layer's nearobjs pass.
const ASF4_DONESND: u8 = 0x02;
/// C `AFEXP` (src/variables.h:69).
const AFEXP: u8 = 1;
/// C `ASF3_REALOBJ` value (src/game/obj.h) — matches sf_game::alien.
const ASF3_REALOBJ: u8 = sf_game::alien::ASF3_REALOBJ;
/// C `ACF_FIRSTFRAME` — matches sf_game::alien.
const ACF_FIRSTFRAME: u8 = sf_game::alien::ACF_FIRSTFRAME;

enum StreamSource {
    StarFox(NativePlayer),
    StarFox2(Sf2NativePlayer),
}

fn sf2_music_cue(mode: sf2_game::GameMode, mission_visit: sf2_game::MissionVisit) -> Sf2MusicCue {
    use sf2_game::{GameMode, IntroPhase, MissionVisit};

    match mode {
        GameMode::Intro(IntroPhase::Boot)
        | GameMode::Intro(IntroPhase::ArgonautLogo)
        | GameMode::Intro(IntroPhase::NintendoLogo) => Sf2MusicCue::LogoPresentation,
        GameMode::Intro(
            IntroPhase::Formation | IntroPhase::TitleReveal | IntroPhase::TitleSplash,
        )
        | GameMode::Title
        | GameMode::Records => Sf2MusicCue::FormationAndTitle,
        GameMode::Briefing => Sf2MusicCue::AndrossBriefing,
        GameMode::StrategicMap => Sf2MusicCue::StrategicMap,
        GameMode::PilotSelection => Sf2MusicCue::PilotSelection,
        GameMode::Mission => match mission_visit {
            MissionVisit::OpeningEngagement
            | MissionVisit::Reengagement
            | MissionVisit::MissileInterception => Sf2MusicCue::OpenSpaceCombat,
            MissionVisit::FighterIntercept => Sf2MusicCue::FighterIntercept,
            MissionVisit::TitaniaBase => Sf2MusicCue::TitaniaBase,
            MissionVisit::MacbethBase => Sf2MusicCue::MacbethBase,
            MissionVisit::EladardBase => Sf2MusicCue::EladardBase,
            MissionVisit::MeteorBase => Sf2MusicCue::MeteorBase,
            MissionVisit::FortunaBase => Sf2MusicCue::FortunaBase,
            MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
                Sf2MusicCue::BattleCarrier
            }
            MissionVisit::MirageDragon => Sf2MusicCue::MirageDragon,
            MissionVisit::PigmaDuel
            | MissionVisit::LeonDuel
            | MissionVisit::RecurringAttackers
            | MissionVisit::LeonPressure
            | MissionVisit::FinalPursuer
            | MissionVisit::WolfBlockade => Sf2MusicCue::RivalEncounter,
            MissionVisit::AstropolisAssault => Sf2MusicCue::AstropolisAssault,
        },
        GameMode::GameOver | GameMode::Results => Sf2MusicCue::GameOverAndContinue,
        GameMode::Ending => Sf2MusicCue::CreditsAndEnding,
    }
}

fn sf2_engine_cue(mode: sf2_game::GameMode, form: sf2_game::PlayerCraftForm) -> Sf2EngineCue {
    if mode != sf2_game::GameMode::Mission || form == sf2_game::PlayerCraftForm::Walker {
        Sf2EngineCue::Silent
    } else {
        Sf2EngineCue::Flight
    }
}

fn sf2_sound_bank(mission_visit: sf2_game::MissionVisit) -> Sf2SoundBank {
    use sf2_game::MissionVisit;

    match mission_visit {
        MissionVisit::OpeningEngagement
        | MissionVisit::Reengagement
        | MissionVisit::MissileInterception => Sf2SoundBank::OpenSpaceCombat,
        MissionVisit::FighterIntercept => Sf2SoundBank::FighterIntercept,
        MissionVisit::TitaniaBase => Sf2SoundBank::TitaniaBase,
        MissionVisit::MacbethBase => Sf2SoundBank::MacbethBase,
        MissionVisit::EladardBase => Sf2SoundBank::EladardBase,
        MissionVisit::MeteorBase => Sf2SoundBank::MeteorBase,
        MissionVisit::FortunaBase => Sf2SoundBank::FortunaBase,
        MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
            Sf2SoundBank::BattleCarrier
        }
        MissionVisit::MirageDragon => Sf2SoundBank::MirageDragon,
        MissionVisit::PigmaDuel
        | MissionVisit::LeonDuel
        | MissionVisit::RecurringAttackers
        | MissionVisit::LeonPressure
        | MissionVisit::FinalPursuer
        | MissionVisit::WolfBlockade => Sf2SoundBank::RivalEncounter,
        MissionVisit::AstropolisAssault => Sf2SoundBank::AstropolisAssault,
    }
}

fn sf2_sound_pilot(pilot: sf2_game::Pilot) -> Sf2SoundPilot {
    match pilot {
        sf2_game::Pilot::Fox => Sf2SoundPilot::Fox,
        sf2_game::Pilot::Falco => Sf2SoundPilot::Falco,
        sf2_game::Pilot::Peppy => Sf2SoundPilot::Peppy,
        sf2_game::Pilot::Slippy => Sf2SoundPilot::Slippy,
        sf2_game::Pilot::Miyu => Sf2SoundPilot::Miyu,
        sf2_game::Pilot::Fay => Sf2SoundPilot::Fay,
    }
}

fn sf2_sound_effect(event: sf2_game::SoundEvent) -> Sf2SoundEffect {
    match event {
        sf2_game::SoundEvent::RapidLaser => Sf2SoundEffect::RapidLaser,
        sf2_game::SoundEvent::ChargedLaser => Sf2SoundEffect::ChargedLaser,
        sf2_game::SoundEvent::HostileLaser => Sf2SoundEffect::HostileLaser,
        sf2_game::SoundEvent::RadioMessageOpen => Sf2SoundEffect::RadioMessageOpen,
        sf2_game::SoundEvent::RadioMessageClose => Sf2SoundEffect::RadioMessageClose,
    }
}

fn sf2_charge_cue(sound: sf2_game::ChargeSound) -> Sf2ChargeCue {
    match sound {
        sf2_game::ChargeSound::Silent => Sf2ChargeCue::Silent,
        sf2_game::ChargeSound::Building => Sf2ChargeCue::Building,
        sf2_game::ChargeSound::Ready => Sf2ChargeCue::Ready,
    }
}

fn sf2_spatial_distance(distance: sf2_game::SpatialDistance) -> Sf2SpatialDistance {
    match distance {
        sf2_game::SpatialDistance::Close => Sf2SpatialDistance::Close,
        sf2_game::SpatialDistance::Near => Sf2SpatialDistance::Near,
        sf2_game::SpatialDistance::Far => Sf2SpatialDistance::Far,
        sf2_game::SpatialDistance::Distant => Sf2SpatialDistance::Distant,
    }
}

fn sf2_stereo_position(position: sf2_game::StereoPosition) -> Sf2StereoPosition {
    match position {
        sf2_game::StereoPosition::Left => Sf2StereoPosition::Left,
        sf2_game::StereoPosition::Center => Sf2StereoPosition::Center,
        sf2_game::StereoPosition::Right => Sf2StereoPosition::Right,
    }
}

fn sf2_spatial_cue(sound: sf2_game::SpatialSound) -> Sf2SpatialCue {
    match sound.sound {
        sf2_game::SpatialLoop::CapitalEngine => Sf2SpatialCue::CapitalEngine {
            distance: sf2_spatial_distance(sound.distance),
            position: sf2_stereo_position(sound.position),
        },
    }
}

/// The audio-thread callback: pull samples from the selected game's native
/// 32 kHz native source.
struct NativeStreamCallback {
    source: StreamSource,
    scratch: Vec<i16>,
}

impl AudioCallback<i16> for NativeStreamCallback {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        // `requested` is in i16 samples; keep whole stereo frames.
        let mut n = requested.max(0) as usize;
        n += n & 1;
        if n == 0 {
            return;
        }
        self.scratch.resize(n, 0);
        match &mut self.source {
            StreamSource::StarFox(player) => player.generate(&mut self.scratch),
            StreamSource::StarFox2(player) => player.generate(&mut self.scratch),
        }
        let _ = stream.put_data_i16(&self.scratch);
    }
}

/// Semantic sound backend over the shared native mixer.
struct NativeBackend {
    player: NativePlayer,
    reported_errors: HashSet<String>,
}

impl NativeBackend {
    fn report(&mut self, result: Result<(), NativeAudioError>) {
        if let Err(error) = result {
            let message = error.to_string();
            if self.reported_errors.insert(message.clone()) {
                eprintln!("Audio: {message}");
            }
        }
    }
}

impl SoundBackend for NativeBackend {
    fn set_engine_sound(&mut self, sound: u8) {
        let result = self.player.set_engine_sound(sound);
        self.report(result);
    }
    fn set_ambient_sound(&mut self, sound: u8) {
        let result = self.player.set_ambient_sound(sound);
        self.report(result);
    }
    fn play_effect(&mut self, effect: u8) {
        let result = self.player.play_effect(effect);
        self.report(result);
    }
    fn effect_consumed(&mut self, effect: u8) -> bool {
        self.player.effect_consumed(effect)
    }
    fn clear_effect_acknowledgement(&mut self) {
        self.player.clear_effect_acknowledgement();
    }
    fn start_music(&mut self, cue: u8) {
        let result = self.player.start_music(cue);
        self.report(result);
    }
    fn load_track(&mut self, track: u8) {
        self.player.load_track(track);
    }
    fn set_paused(&mut self, paused: bool) {
        self.player.set_paused(paused);
    }
}

pub struct AudioSys {
    /// Keeps the SDL stream (and its callback) alive; None when the audio
    /// device could not be opened (CI/dummy driver) — the game still runs.
    _stream: Option<AudioStreamWithCallback<NativeStreamCallback>>,
    backend: NativeBackend,
    sound: Sound,
    sf2_player: Option<Sf2NativePlayer>,
    sf2_music: Option<Sf2MusicCue>,
    sf2_engine: Option<(Sf2SoundBank, Sf2SoundPilot, Sf2EngineCue)>,
    sf2_charge: Option<(Sf2SoundBank, Sf2SoundPilot, Sf2ChargeCue)>,
    sf2_spatial: Option<(Sf2SoundBank, Sf2SoundPilot, sf2_game::SpatialSound)>,
}

impl AudioSys {
    /// C `Audio_Init` (src/audio/audio.c) + `Sound_Init` (boot.c Game_Init).
    /// A failed device open is a warning, not an error (dummy-audio guard).
    pub fn new(sdl: &sdl3::Sdl, asset_dir: PathBuf) -> Result<AudioSys, NativeAudioError> {
        let player = NativePlayer::new(&asset_dir);
        player.validate_star_fox_assets()?;
        Ok(Self::with_source(
            sdl,
            asset_dir,
            player.clone(),
            StreamSource::StarFox(player),
            None,
        ))
    }

    pub fn new_sf2(sdl: &sdl3::Sdl, asset_dir: PathBuf) -> Result<AudioSys, NativeAudioError> {
        let sf2_player = Sf2NativePlayer::new(&asset_dir);
        sf2_player.validate_assets()?;
        let backend_player = NativePlayer::new(&asset_dir);
        Ok(Self::with_source(
            sdl,
            asset_dir,
            backend_player,
            StreamSource::StarFox2(sf2_player.clone()),
            Some(sf2_player),
        ))
    }

    fn with_source(
        sdl: &sdl3::Sdl,
        _asset_dir: PathBuf,
        player: NativePlayer,
        source: StreamSource,
        sf2_player: Option<Sf2NativePlayer>,
    ) -> AudioSys {
        let stream = match sdl.audio() {
            Ok(audio) => {
                let spec = AudioSpec {
                    freq: Some(AUDIO_RATE),
                    channels: Some(2),
                    format: Some(AudioFormat::s16_sys()),
                };
                let cb = NativeStreamCallback {
                    source,
                    scratch: Vec::new(),
                };
                match audio.open_playback_stream(&spec, cb) {
                    Ok(stream) => match stream.resume() {
                        Ok(()) => {
                            println!("Audio: native PCM stream at {AUDIO_RATE} Hz stereo");
                            Some(stream)
                        }
                        Err(e) => {
                            eprintln!("Audio: stream resume failed: {e} (running silent)");
                            None
                        }
                    },
                    Err(e) => {
                        eprintln!("Audio: open stream failed: {e} (running silent)");
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("Audio: SDL audio init failed: {e} (running silent)");
                None
            }
        };

        let mut backend = NativeBackend {
            player,
            reported_errors: HashSet::new(),
        };
        let mut sound = Sound::new();
        sound.init(&mut backend);
        AudioSys {
            _stream: stream,
            backend,
            sound,
            sf2_player,
            sf2_music: None,
            sf2_engine: None,
            sf2_charge: None,
            sf2_spatial: None,
        }
    }

    pub fn tick_sf2(&mut self, game: &mut sf2_game::Game) {
        let mission_visit = game.state().mission.visit;
        let cue = sf2_music_cue(game.mode(), mission_visit);
        let Some(player) = &self.sf2_player else {
            return;
        };

        if self.sf2_music != Some(cue) {
            let result = player.start_music(cue);
            if result.is_ok() {
                self.sf2_music = Some(cue);
            }
            self.backend.report(result);
        }

        let bank = sf2_sound_bank(mission_visit);
        let pilot = sf2_sound_pilot(game.active_pilot());
        let engine = sf2_engine_cue(game.mode(), game.state().mission.player_craft_form);
        let engine_state = (bank, pilot, engine);
        if self.sf2_engine != Some(engine_state) {
            let result = player.set_engine(bank, pilot, engine);
            if result.is_ok() {
                self.sf2_engine = Some(engine_state);
            }
            self.backend.report(result);
        }

        let charge = sf2_charge_cue(game.charge_sound());
        let charge_state = (bank, pilot, charge);
        if self.sf2_charge != Some(charge_state) {
            let result = player.set_charge(bank, pilot, charge);
            if result.is_ok() {
                self.sf2_charge = Some(charge_state);
            }
            self.backend.report(result);
        }

        let spatial = game.spatial_sound();
        let spatial_state = spatial.map(|sound| (bank, pilot, sound));
        if self.sf2_spatial != spatial_state {
            let restart = self
                .sf2_spatial
                .zip(spatial_state)
                .is_some_and(|(previous, next)| previous.2.source != next.2.source);
            let result = player.set_spatial(bank, pilot, spatial.map(sf2_spatial_cue), restart);
            if result.is_ok() {
                self.sf2_spatial = spatial_state;
            }
            self.backend.report(result);
        }

        for event in game.take_sound_events().into_iter().flatten() {
            self.backend
                .report(player.play_effect(bank, pilot, sf2_sound_effect(event)));
        }
    }

    /// Quiesce the SDL callback before its Rust userdata is freed.
    ///
    /// sdl3 0.18's `AudioStreamWithCallback::drop` releases the callback box
    /// before its `AudioStreamOwner` field destroys the native stream. A
    /// callback racing that drop reads freed userdata and intermittently
    /// corrupts the allocator at process exit. Pause first, then take the
    /// stream lock (which waits for an in-flight callback) before dropping.
    pub fn shutdown(&mut self) {
        let Some(mut stream) = self._stream.take() else {
            return;
        };
        if let Err(e) = stream.pause() {
            eprintln!("Audio: stream pause during shutdown failed: {e}");
        }
        // SDL documents that the stream mutex is held throughout callbacks.
        // Acquiring and releasing it after pausing is therefore a callback
        // completion barrier; no new callback can begin on the paused device.
        if let Some(guard) = stream.lock() {
            drop(guard);
        }
        drop(stream);
    }

    /// Per-tick sound processing, run right after `Shell::tick`:
    /// drain the shell's queued Sound_* calls, then run the
    /// `Sound_Update` (dosounds_l) pass against the live game state.
    pub fn tick(&mut self, shell: &mut Shell, frame: &FrameSnapshot) {
        let state = Self::sound_state(shell, frame);

        // Queued Sound_PlaySE / Sound_PlayMusic / ... calls from the map VM,
        // boot.c, windows.c, strings.c, planets.c call sites.
        let in_gameplay = shell.state() == GameState::Playing;
        for cmd in shell.drain_sound() {
            match cmd {
                SoundCmd::PlaySe(id) => self.sound.play_se(&state, id),
                SoundCmd::MakeSnd { family, x, z } => {
                    // C makesnd (SOUND.ASM:899): band the family's ids against
                    // the live player position. No player -> nothing to key on.
                    if let Some(player) = Self::sound_player(&shell.game) {
                        let fam = Self::pos_family(family);
                        self.sound.make_snd(&state, &player, x, z, fam);
                    }
                }
                SoundCmd::PlayMusic(id) => {
                    self.sound.play_music(&mut self.backend, id, in_gameplay)
                }
                SoundCmd::PlayImmediate(id) => self.sound.play(&mut self.backend, id),
                SoundCmd::StopMusic => self.sound.stop_music(&mut self.backend),
                SoundCmd::PauseSnd(cmd) => self.sound.set_pause_snd(cmd),
                SoundCmd::NoSetPort3(disabled) => self.sound.set_nosetport3(disabled),
            }
        }

        // dosounds_l (nmi.c:73 Sound_Update): gameplay only, matching the C
        // call site inside Nmi_GameTick.
        if in_gameplay {
            let player = Self::sound_player(&shell.game);
            let mut objs = Self::sound_objs(&shell.game);
            self.sound
                .update(&state, player.as_ref(), &mut objs, &mut self.backend);
            // Write back ASF4_DONESND (C sets it directly on the alien).
            for obj in &objs {
                if obj.donesnd && obj.id > 0 {
                    let idx = (obj.id - 1) as usize;
                    if idx < shell.game.objs.aliens.len() {
                        shell.game.objs.aliens[idx].sflags4 |= ASF4_DONESND;
                    }
                }
            }
        }
    }

    /// Play a one-shot SE from the HUD/render path (arrow beep $8A).
    /// Same ring as shell `SoundCmd::PlaySe` — not gated by pause.
    pub fn play_hud_se(&mut self, shell: &Shell, id: u8) {
        let frame = shell.frame();
        let state = Self::sound_state(shell, &frame);
        self.sound.play_se(&state, id);
    }

    /// Assemble `SoundGameState` from shell state (the globals sound.c read).
    fn sound_state(shell: &Shell, frame: &FrameSnapshot) -> SoundGameState {
        let in_game = shell.state() == GameState::Playing;
        let mut mapped = [0u16; 5];
        for (i, &shape_idx) in sf_audio::sound::FORCESND_SHAPE_IDS.iter().enumerate() {
            let table = &shell.game.world.shapes_table;
            if (shape_idx as usize) < table.len() {
                mapped[i] = table[shape_idx as usize];
            }
        }
        SoundGameState {
            in_game,
            player_dead: frame.player_dead,
            player_hp0: frame.player_hp0,
            engine_snd: frame.engine_snd,
            level_finished: frame.level_finished,
            in_a_tunnel: shell.game.vars.in_a_tunnel,
            space_mode: frame.space_mode,
            player_snd_flag: shell.game.vars.player_snd_flag,
            pad1: shell.game.vars.pad1,
            pviewposx: frame.pviewposx,
            new_map: frame.newmap,
            mapped_forcesnd_shapes: mapped,
        }
    }

    /// Map a strat-lane [`PosSndFamilyId`] to the sf-audio `POS_*` id table
    /// (the `*sound_l` L/C/R/mid/far ids of SOUND.ASM:735-897).
    fn pos_family(id: PosSndFamilyId) -> &'static PosSndFamily {
        use sf_audio::sound::*;
        match id {
            PosSndFamilyId::Laser => &POS_LASER,
            PosSndFamilyId::Missile => &POS_MISSILE,
            PosSndFamilyId::HitWall => &POS_HITWALL,
            PosSndFamilyId::MoveWall => &POS_MOVEWALL,
            PosSndFamilyId::RingLaser => &POS_RINGLASER,
            PosSndFamilyId::DoorOpen => &POS_DOOROPEN,
            PosSndFamilyId::DoorClose => &POS_DOORCLOSE,
            PosSndFamilyId::EnemyUpSea => &POS_ENEMYUPSEA,
            PosSndFamilyId::EnemyDownSea => &POS_ENEMYDOWNSEA,
            PosSndFamilyId::DestBoss => &POS_DESTBOSS,
            PosSndFamilyId::DestEnemy => &POS_DESTENEMY,
            PosSndFamilyId::DamEnemy => &POS_DAMENEMY,
            PosSndFamilyId::EnemyBattry => &POS_ENEMYBATTRY,
            PosSndFamilyId::SeparateMissile => &POS_SEPARATEMISSILE,
        }
    }

    /// C `sound_get_player`.
    fn sound_player(game: &Game) -> Option<SoundPlayer> {
        game.objs.player().map(|al| SoundPlayer {
            first_frame: al.collflags & ACF_FIRSTFRAME != 0,
            worldx: al.worldx,
            worldz: al.worldz,
        })
    }

    /// Active list in list order (C walks g_active_list).
    fn sound_objs(game: &Game) -> Vec<SoundObj> {
        game.objs
            .active_indices()
            .into_iter()
            .map(|idx| {
                let al = &game.objs.aliens[idx as usize];
                SoundObj {
                    id: idx + 1,
                    shape: al.shape,
                    exploding: al.flags & AFEXP != 0,
                    snd1: al.snd1,
                    snd2: al.snd2,
                    worldx: al.worldx,
                    worldz: al.worldz,
                    hp: al.hp,
                    realobj: al.sflags3 & ASF3_REALOBJ != 0,
                    donesnd: al.sflags4 & ASF4_DONESND != 0,
                }
            })
            .collect()
    }
}

impl Drop for AudioSys {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_audio::sound::*;

    /// Every strat-lane family selector maps to the matching SOUND.ASM
    /// `*sound_l` id table (findings F1-F4 rely on Door*/Sea being correct).
    #[test]
    fn pos_family_maps_every_variant() {
        use PosSndFamilyId::*;
        assert_eq!(AudioSys::pos_family(Laser), &POS_LASER);
        assert_eq!(AudioSys::pos_family(Missile), &POS_MISSILE);
        assert_eq!(AudioSys::pos_family(HitWall), &POS_HITWALL);
        assert_eq!(AudioSys::pos_family(MoveWall), &POS_MOVEWALL);
        assert_eq!(AudioSys::pos_family(RingLaser), &POS_RINGLASER);
        assert_eq!(AudioSys::pos_family(DoorOpen), &POS_DOOROPEN);
        assert_eq!(AudioSys::pos_family(DoorClose), &POS_DOORCLOSE);
        assert_eq!(AudioSys::pos_family(EnemyUpSea), &POS_ENEMYUPSEA);
        assert_eq!(AudioSys::pos_family(EnemyDownSea), &POS_ENEMYDOWNSEA);
        assert_eq!(AudioSys::pos_family(DestBoss), &POS_DESTBOSS);
        assert_eq!(AudioSys::pos_family(DestEnemy), &POS_DESTENEMY);
        assert_eq!(AudioSys::pos_family(DamEnemy), &POS_DAMENEMY);
        assert_eq!(AudioSys::pos_family(EnemyBattry), &POS_ENEMYBATTRY);
        assert_eq!(AudioSys::pos_family(SeparateMissile), &POS_SEPARATEMISSILE);
    }

    /// End-to-end of the F1/F3 path: a mapped family routed through
    /// Sound::make_snd bands by distance (near-centre vs far).
    #[test]
    fn make_snd_routes_mapped_family_by_distance() {
        let mut snd = Sound::new();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        let open = AudioSys::pos_family(PosSndFamilyId::DoorOpen);
        // Near-centre door-open -> $54; far -> $55.
        assert_eq!(snd.make_snd(&st, &player, 0, 50, open), Some(0x54));
        assert_eq!(snd.make_snd(&st, &player, 0, 2500, open), Some(0x55));
    }

    #[test]
    fn sf2_game_over_selects_its_verified_music_program() {
        assert_eq!(
            sf2_music_cue(
                sf2_game::GameMode::GameOver,
                sf2_game::MissionVisit::OpeningEngagement,
            ),
            Sf2MusicCue::GameOverAndContinue
        );
    }

    #[test]
    fn sf2_ending_selects_its_verified_staff_roll_program() {
        assert_eq!(
            sf2_music_cue(
                sf2_game::GameMode::Ending,
                sf2_game::MissionVisit::OpeningEngagement,
            ),
            Sf2MusicCue::CreditsAndEnding
        );
    }

    #[test]
    fn sf2_every_mission_visit_selects_its_verified_music_program() {
        use sf2_game::MissionVisit::*;

        let expected = [
            (OpeningEngagement, Sf2MusicCue::OpenSpaceCombat),
            (Reengagement, Sf2MusicCue::OpenSpaceCombat),
            (MissileInterception, Sf2MusicCue::OpenSpaceCombat),
            (FighterIntercept, Sf2MusicCue::FighterIntercept),
            (PigmaDuel, Sf2MusicCue::RivalEncounter),
            (EladardBase, Sf2MusicCue::EladardBase),
            (TitaniaBase, Sf2MusicCue::TitaniaBase),
            (MacbethBase, Sf2MusicCue::MacbethBase),
            (MeteorBase, Sf2MusicCue::MeteorBase),
            (FirstBattleCarrier, Sf2MusicCue::BattleCarrier),
            (SecondBattleCarrier, Sf2MusicCue::BattleCarrier),
            (LeonDuel, Sf2MusicCue::RivalEncounter),
            (MirageDragon, Sf2MusicCue::MirageDragon),
            (RecurringAttackers, Sf2MusicCue::RivalEncounter),
            (LeonPressure, Sf2MusicCue::RivalEncounter),
            (FinalPursuer, Sf2MusicCue::RivalEncounter),
            (WolfBlockade, Sf2MusicCue::RivalEncounter),
            (AstropolisAssault, Sf2MusicCue::AstropolisAssault),
        ];

        for (visit, cue) in expected {
            assert_eq!(sf2_music_cue(sf2_game::GameMode::Mission, visit), cue);
        }
    }

    #[test]
    fn sf2_every_mission_visit_selects_its_verified_sound_bank() {
        use sf2_game::MissionVisit::*;

        let expected = [
            (OpeningEngagement, Sf2SoundBank::OpenSpaceCombat),
            (Reengagement, Sf2SoundBank::OpenSpaceCombat),
            (MissileInterception, Sf2SoundBank::OpenSpaceCombat),
            (FighterIntercept, Sf2SoundBank::FighterIntercept),
            (PigmaDuel, Sf2SoundBank::RivalEncounter),
            (EladardBase, Sf2SoundBank::EladardBase),
            (TitaniaBase, Sf2SoundBank::TitaniaBase),
            (MacbethBase, Sf2SoundBank::MacbethBase),
            (MeteorBase, Sf2SoundBank::MeteorBase),
            (FirstBattleCarrier, Sf2SoundBank::BattleCarrier),
            (SecondBattleCarrier, Sf2SoundBank::BattleCarrier),
            (LeonDuel, Sf2SoundBank::RivalEncounter),
            (MirageDragon, Sf2SoundBank::MirageDragon),
            (RecurringAttackers, Sf2SoundBank::RivalEncounter),
            (LeonPressure, Sf2SoundBank::RivalEncounter),
            (FinalPursuer, Sf2SoundBank::RivalEncounter),
            (WolfBlockade, Sf2SoundBank::RivalEncounter),
            (AstropolisAssault, Sf2SoundBank::AstropolisAssault),
        ];

        for (visit, bank) in expected {
            assert_eq!(sf2_sound_bank(visit), bank);
        }
    }

    #[test]
    fn sf2_every_pilot_selects_its_verified_sound_variant() {
        use sf2_game::Pilot;

        let expected = [
            (Pilot::Fox, Sf2SoundPilot::Fox),
            (Pilot::Falco, Sf2SoundPilot::Falco),
            (Pilot::Peppy, Sf2SoundPilot::Peppy),
            (Pilot::Slippy, Sf2SoundPilot::Slippy),
            (Pilot::Miyu, Sf2SoundPilot::Miyu),
            (Pilot::Fay, Sf2SoundPilot::Fay),
        ];

        for (pilot, sound_pilot) in expected {
            assert_eq!(sf2_sound_pilot(pilot), sound_pilot);
        }
    }

    #[test]
    fn sf2_pilot_selection_and_results_do_not_reuse_unrelated_music() {
        use sf2_game::{GameMode, MissionVisit};

        assert_eq!(
            sf2_music_cue(GameMode::PilotSelection, MissionVisit::OpeningEngagement),
            Sf2MusicCue::PilotSelection,
        );
        assert_eq!(
            sf2_music_cue(GameMode::Results, MissionVisit::AstropolisAssault),
            Sf2MusicCue::GameOverAndContinue,
        );
    }

    #[test]
    fn sf2_flight_engine_stops_outside_flight_missions() {
        use sf2_game::{GameMode, PlayerCraftForm};

        assert_eq!(
            sf2_engine_cue(GameMode::Mission, PlayerCraftForm::Flight),
            Sf2EngineCue::Flight,
        );
        assert_eq!(
            sf2_engine_cue(GameMode::Mission, PlayerCraftForm::Walker),
            Sf2EngineCue::Silent,
        );
        assert_eq!(
            sf2_engine_cue(GameMode::StrategicMap, PlayerCraftForm::Flight),
            Sf2EngineCue::Silent,
        );
    }

    #[test]
    fn sf2_typed_sound_events_route_to_native_effects() {
        use sf2_game::SoundEvent;

        let expected = [
            (SoundEvent::RapidLaser, Sf2SoundEffect::RapidLaser),
            (SoundEvent::ChargedLaser, Sf2SoundEffect::ChargedLaser),
            (SoundEvent::HostileLaser, Sf2SoundEffect::HostileLaser),
            (
                SoundEvent::RadioMessageOpen,
                Sf2SoundEffect::RadioMessageOpen,
            ),
            (
                SoundEvent::RadioMessageClose,
                Sf2SoundEffect::RadioMessageClose,
            ),
        ];
        for (event, effect) in expected {
            assert_eq!(sf2_sound_effect(event), effect);
        }
    }

    #[test]
    fn sf2_typed_charge_state_routes_to_its_dedicated_layer() {
        use sf2_game::ChargeSound;

        assert_eq!(sf2_charge_cue(ChargeSound::Silent), Sf2ChargeCue::Silent);
        assert_eq!(
            sf2_charge_cue(ChargeSound::Building),
            Sf2ChargeCue::Building
        );
        assert_eq!(sf2_charge_cue(ChargeSound::Ready), Sf2ChargeCue::Ready);
    }

    #[test]
    fn sf2_spatial_state_routes_to_the_dedicated_positional_layer() {
        let distances = [
            (sf2_game::SpatialDistance::Close, Sf2SpatialDistance::Close),
            (sf2_game::SpatialDistance::Near, Sf2SpatialDistance::Near),
            (sf2_game::SpatialDistance::Far, Sf2SpatialDistance::Far),
            (
                sf2_game::SpatialDistance::Distant,
                Sf2SpatialDistance::Distant,
            ),
        ];
        for (source, output) in distances {
            assert_eq!(sf2_spatial_distance(source), output);
        }

        let positions = [
            (sf2_game::StereoPosition::Left, Sf2StereoPosition::Left),
            (sf2_game::StereoPosition::Center, Sf2StereoPosition::Center),
            (sf2_game::StereoPosition::Right, Sf2StereoPosition::Right),
        ];
        for (source, output) in positions {
            assert_eq!(sf2_stereo_position(source), output);
        }
    }
}
