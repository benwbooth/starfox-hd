//! Audio bridge: SDL3 playback stream driving the sf-audio SPC player,
//! plus the game-side Sound layer wired to shell state each tick.
//!
//! Port (C oracle): `src/audio/audio.c` (SDL audio device + callback ->
//! `SpcPlayer_Generate`) and the `src/game/sound.c` call sites in
//! `Nmi_GameTick`/boot.c (here: `AudioSys::tick`). The SPC always runs at
//! its native 32000 Hz stereo; SDL3 resamples to the device rate.

use std::path::PathBuf;

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};
use sf_audio::player::SpcPlayer;
use sf_audio::sound::{Sound, SoundBackend, SoundGameState, SoundObj, SoundPlayer};
use sf_game::game::Game;
use sf_game::shell::{FrameSnapshot, GameState, Shell, SoundCmd};

/// SPC native output rate (C SPC_SAMPLE_RATE).
const SPC_RATE: i32 = 32000;

/// C `ASF4_DONESND` (src/game/obj.h:116) — write-back flag from the sound
/// layer's nearobjs pass.
const ASF4_DONESND: u8 = 0x02;
/// C `AFEXP` (src/variables.h:69).
const AFEXP: u8 = 1;
/// C `ASF3_REALOBJ` value (src/game/obj.h) — matches sf_game::alien.
const ASF3_REALOBJ: u8 = sf_game::alien::ASF3_REALOBJ;
/// C `ACF_FIRSTFRAME` — matches sf_game::alien.
const ACF_FIRSTFRAME: u8 = sf_game::alien::ACF_FIRSTFRAME;

/// The audio-thread callback: pull samples from the shared SPC player.
/// C oracle: `audio_callback` in src/audio/audio.c.
struct SpcStreamCallback {
    player: SpcPlayer,
    scratch: Vec<i16>,
}

impl AudioCallback<i16> for SpcStreamCallback {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        // `requested` is in i16 samples; keep whole stereo frames.
        let mut n = requested.max(0) as usize;
        n += n & 1;
        if n == 0 {
            return;
        }
        self.scratch.resize(n, 0);
        self.player.generate(&mut self.scratch);
        let _ = stream.put_data_i16(&self.scratch);
    }
}

/// SoundBackend over the shared SpcPlayer handle (C SpcPlayer_* calls).
struct SpcBackend {
    player: SpcPlayer,
    asset_dir: PathBuf,
}

impl SoundBackend for SpcBackend {
    fn write_port(&mut self, port: u8, value: u8) {
        self.player.write_port(port, value);
    }
    fn read_port(&mut self, port: u8) -> u8 {
        self.player.read_port(port)
    }
    fn start_bgm(&mut self, cmd: u8) {
        self.player.start_bgm(cmd);
    }
    fn load_track(&mut self, track_id: u8) {
        if let Err(e) = self.player.load_track(track_id, &self.asset_dir) {
            eprintln!("Audio: load_track({track_id}) failed: {e}");
        }
    }
}

pub struct AudioSys {
    /// Keeps the SDL stream (and its callback) alive; None when the audio
    /// device could not be opened (CI/dummy driver) — the game still runs.
    _stream: Option<AudioStreamWithCallback<SpcStreamCallback>>,
    backend: SpcBackend,
    sound: Sound,
}

impl AudioSys {
    /// C `Audio_Init` (src/audio/audio.c) + `Sound_Init` (boot.c Game_Init).
    /// A failed device open is a warning, not an error (dummy-audio guard).
    pub fn new(sdl: &sdl3::Sdl, asset_dir: PathBuf) -> AudioSys {
        let player = SpcPlayer::new();
        let stream = match sdl.audio() {
            Ok(audio) => {
                let spec = AudioSpec {
                    freq: Some(SPC_RATE),
                    channels: Some(2),
                    format: Some(AudioFormat::s16_sys()),
                };
                let cb = SpcStreamCallback {
                    player: player.clone(),
                    scratch: Vec::new(),
                };
                match audio.open_playback_stream(&spec, cb) {
                    Ok(stream) => match stream.resume() {
                        Ok(()) => {
                            println!("Audio: SPC stream at {SPC_RATE} Hz stereo");
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

        let mut backend = SpcBackend { player, asset_dir };
        let mut sound = Sound::new();
        sound.init(&mut backend);
        AudioSys {
            _stream: stream,
            backend,
            sound,
        }
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
                SoundCmd::PlayMusic(id) => {
                    self.sound.play_music(&mut self.backend, id, in_gameplay)
                }
                SoundCmd::PlayImmediate(id) => self.sound.play(&mut self.backend, id),
                SoundCmd::StopMusic => self.sound.stop_music(&mut self.backend),
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
            in_a_tunnel: 0, // TODO(sf-strat): g_inatunnel is strat-lane state
            space_mode: frame.space_mode,
            player_snd_flag: 0, // TODO(sf-strat): g_playersndflag
            pad1: shell.game.vars.pad1,
            pviewposx: frame.pviewposx,
            new_map: frame.newmap,
            mapped_forcesnd_shapes: mapped,
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
