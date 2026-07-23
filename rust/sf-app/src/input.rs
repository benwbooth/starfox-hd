//! Pad input: keyboard map + SDL3 gamepads + SF_AUTOPLAY script.
//!
//! Port (C oracle): `src/sf_rtl.c` — `key_map[]` (sf_rtl.c:25), gamepad
//! button map (`ReadGamepad`, sf_rtl.c:91), the scripted `AutoplayPad`
//! (sf_rtl.c:123) and the `SfRtl_BeginFrame` pad latch (sf_rtl.c:142).
//!
//! Differences from the C (intentional): the C opened only the FIRST game
//! controller; here every connected gamepad is opened and OR-ed together
//! (standard SDL3 gamepad mapping — this is the Steam Controller 2 path).

use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::keyboard::{KeyboardState, Scancode};
use sf2_game::{Game as Sf2Game, GameMode as Sf2Mode, GameOverPhase, StrategicMapPhase};
use sf_core::pad;

/// One keyboard binding (C `key_map[]`, sf_rtl.c:25-42).
const KEY_MAP: &[(Scancode, u16)] = &[
    (Scancode::X, pad::A),           // A button
    (Scancode::Z, pad::B),           // B button
    (Scancode::A, pad::X),           // X button
    (Scancode::S, pad::Y),           // Y button
    (Scancode::Q, pad::TLEFT),       // L shoulder
    (Scancode::W, pad::TRIGHT),      // R shoulder
    (Scancode::Return, pad::START),  // Start
    (Scancode::Space, pad::START),   // Start (alternate)
    (Scancode::RShift, pad::SELECT), // Select
    (Scancode::Up, pad::UP),
    (Scancode::Down, pad::DOWN),
    (Scancode::Left, pad::LEFT),
    (Scancode::Right, pad::RIGHT),
];

/// Analog stick deadzone (C `DEADZONE`, sf_rtl.c:111).
const STICK_DEADZONE: i16 = 8000;
const SF2_FRONT_END_CONFIRM_TICKS: [u32; 6] = [850, 880, 910, 980, 1_010, 1_040];
const SF2_FRONT_END_START_TICKS: [u32; 4] = [0, 60, 120, 180];
const SF2_MISSION_INPUT_START_TICK: u32 = 1_050;
const SF2_DYNAMIC_CONFIRM_CADENCE_TICKS: u32 = 2;

pub struct Input {
    /// C `g_pad1` / `g_pad1_new` / `g_pad1_prev`.
    pub pad1: u16,
    pub pad1_new: u16,
    pub pad1_prev: u16,
    /// C `g_frame_count` (ticks, not render frames).
    pub frame_count: u32,
    /// SF_AUTOPLAY=1 (C AutoplayPad enable latch).
    autoplay: bool,
    /// The SF2 native campaign needs its own verified front-end sequence.
    sf2_autoplay: bool,
    /// Optional verification hold on the strategic map after N sorties.
    sf2_autoplay_pause_after_sorties: Option<u16>,
    /// All open gamepads, keyed by joystick instance id.
    pub gamepads: Vec<Gamepad>,
}

impl Input {
    pub fn new(sf2_autoplay: bool) -> Self {
        let autoplay = std::env::var("SF_AUTOPLAY")
            .map(|v| v.starts_with('1'))
            .unwrap_or(false);
        let sf2_autoplay_pause_after_sorties = std::env::var("SF2_AUTOPLAY_PAUSE_AFTER_SORTIES")
            .ok()
            .and_then(|value| value.parse().ok());
        Input {
            pad1: 0,
            pad1_new: 0,
            pad1_prev: 0,
            frame_count: 0,
            autoplay,
            sf2_autoplay,
            sf2_autoplay_pause_after_sorties,
            gamepads: Vec::new(),
        }
    }

    /// Open every connected gamepad (standard mapping). C opened only the
    /// first controller (sf_rtl.c:56); we OR all of them.
    pub fn open_all_gamepads(&mut self, subsystem: &sdl3::GamepadSubsystem) {
        let ids = match subsystem.gamepads() {
            Ok(ids) => ids,
            Err(e) => {
                eprintln!("Gamepad enumeration failed: {e}");
                return;
            }
        };
        for id in ids {
            self.add_gamepad(subsystem, id);
        }
        if self.gamepads.is_empty() {
            println!("Controller: none connected");
        }
    }

    pub fn add_gamepad(
        &mut self,
        subsystem: &sdl3::GamepadSubsystem,
        id: sdl3::joystick::JoystickId,
    ) {
        if self.gamepads.iter().any(|g| g.id().ok() == Some(id)) {
            return;
        }
        match subsystem.open(id) {
            Ok(gp) => {
                println!(
                    "Controller connected: {}",
                    gp.name().unwrap_or_else(|| "(unnamed)".into())
                );
                self.gamepads.push(gp);
            }
            Err(e) => eprintln!("Controller open failed: {e}"),
        }
    }

    pub fn remove_gamepad(&mut self, id: sdl3::joystick::JoystickId) {
        let before = self.gamepads.len();
        self.gamepads.retain(|g| g.id().ok() != Some(id));
        if self.gamepads.len() != before {
            println!("Controller disconnected");
        }
    }

    /// C `ReadKeyboard` (sf_rtl.c:80).
    fn read_keyboard(keys: &KeyboardState) -> u16 {
        let mut pad = 0u16;
        for &(sc, bit) in KEY_MAP {
            if keys.is_scancode_pressed(sc) {
                pad |= bit;
            }
        }
        pad
    }

    /// C `ReadGamepad` (sf_rtl.c:91) over every open pad.
    ///
    /// SDL positional buttons are Xbox-layout (South=bottom, East=right,
    /// West=left, North=top). The SNES face layout is rotated — its A/B and
    /// X/Y sit in swapped physical positions vs Xbox:
    /// ```
    ///     SNES            SDL position
    ///      X               North(top)
    ///    Y   A          West(l)  East(r)
    ///      B               South(bottom)
    /// ```
    /// So we map by PHYSICAL position: bottom->B, right->A, left->Y, top->X.
    /// Gameplay assigns the actions; the SF2 retail trace confirms that the
    /// bottom B input fires its player laser.
    fn read_gamepads(&self) -> u16 {
        let mut pad = 0u16;
        for gp in &self.gamepads {
            if gp.button(Button::South) {
                pad |= pad::B; // bottom = SNES B (brake)
            }
            if gp.button(Button::East) {
                pad |= pad::A; // right = SNES A (nova bomb)
            }
            if gp.button(Button::West) {
                pad |= pad::Y; // left = SNES Y (fire laser)
            }
            if gp.button(Button::North) {
                pad |= pad::X; // top = SNES X (boost)
            }
            if gp.button(Button::LeftShoulder) {
                pad |= pad::TLEFT;
            }
            if gp.button(Button::RightShoulder) {
                pad |= pad::TRIGHT;
            }
            if gp.button(Button::Start) {
                pad |= pad::START;
            }
            if gp.button(Button::Back) {
                pad |= pad::SELECT;
            }
            if gp.button(Button::DPadUp) {
                pad |= pad::UP;
            }
            if gp.button(Button::DPadDown) {
                pad |= pad::DOWN;
            }
            if gp.button(Button::DPadLeft) {
                pad |= pad::LEFT;
            }
            if gp.button(Button::DPadRight) {
                pad |= pad::RIGHT;
            }
            // Guide button as an extra Start source (some pads route the
            // menu/Start under Guide).
            if gp.button(Button::Guide) {
                pad |= pad::START;
            }

            // Diagnostic: SF_PADDBG=1 prints which buttons/axes the pad
            // actually reports, so an unmapped controller (e.g. Steam
            // Controller 2) can be identified. Throttled to changes.
            if std::env::var("SF_PADDBG").is_ok() {
                use std::fmt::Write as _;
                let mut s = String::new();
                for (b, name) in [
                    (Button::South, "South"),
                    (Button::East, "East"),
                    (Button::West, "West"),
                    (Button::North, "North"),
                    (Button::Start, "Start"),
                    (Button::Back, "Back"),
                    (Button::Guide, "Guide"),
                    (Button::LeftShoulder, "L"),
                    (Button::RightShoulder, "R"),
                    (Button::LeftStick, "LS"),
                    (Button::RightStick, "RS"),
                    (Button::DPadUp, "Up"),
                    (Button::DPadDown, "Down"),
                    (Button::DPadLeft, "Left"),
                    (Button::DPadRight, "Right"),
                ] {
                    if gp.button(b) {
                        let _ = write!(s, "{name} ");
                    }
                }
                for (a, name) in [
                    (Axis::LeftX, "LX"),
                    (Axis::LeftY, "LY"),
                    (Axis::RightX, "RX"),
                    (Axis::RightY, "RY"),
                    (Axis::TriggerLeft, "LT"),
                    (Axis::TriggerRight, "RT"),
                ] {
                    let v = gp.axis(a);
                    if v.abs() > 6000 {
                        let _ = write!(s, "{name}={v} ");
                    }
                }
                if !s.is_empty() {
                    eprintln!("PADDBG: {s}");
                }
            }

            // Analog stick -> dpad with deadzone (sf_rtl.c:109-115).
            // NOTE: the game logic + renderer are proven correct (pad::LEFT ->
            // ship screen-left; see tests/steering.rs + the render-direction
            // test). Any residual left/right inversion is the Steam
            // Controller's analog X axis, which SDL doesn't map for this
            // hardware — the real fix is a proper SC2 gamepad mapping, not a
            // sign flip here. The d-pad path is correct.
            let lx = gp.axis(Axis::LeftX);
            let ly = gp.axis(Axis::LeftY);
            // Garbage-axis guard: a controller SDL hasn't correctly mapped
            // (e.g. the Steam Controller 2) reports multiple axes pegged to
            // opposite extremes at once, which real sticks physically can't
            // do — that noise would spam the dpad and stomp real input.
            // When detected, ignore stick input this frame (digital dpad +
            // keyboard still work).
            let rx = gp.axis(Axis::RightX);
            let ry = gp.axis(Axis::RightY);
            let saturated = |v: i16| v.abs() > 30000;
            let axes_garbage = (saturated(lx) && saturated(rx))
                || (saturated(ly) && saturated(ry))
                || (saturated(lx) && saturated(ly) && saturated(rx) && saturated(ry));
            if !axes_garbage {
                if lx < -STICK_DEADZONE {
                    pad |= pad::LEFT;
                }
                if lx > STICK_DEADZONE {
                    pad |= pad::RIGHT;
                }
                if ly < -STICK_DEADZONE {
                    pad |= pad::UP;
                }
                if ly > STICK_DEADZONE {
                    pad |= pad::DOWN;
                }
            }
        }
        pad
    }

    /// C `AutoplayPad` (sf_rtl.c:123) for SF1, plus a separate SF2 schedule
    /// that follows the independently traced retail campaign hierarchy.
    fn autoplay_pad(&self, sf2_game: Option<&Sf2Game>) -> u16 {
        if !self.autoplay {
            return 0;
        }
        let t = self.frame_count;
        if self.sf2_autoplay {
            if SF2_FRONT_END_START_TICKS.contains(&t) {
                return pad::START;
            }
            if SF2_FRONT_END_CONFIRM_TICKS.contains(&t) {
                return pad::B;
            }
            if let Some(game) = sf2_game {
                let dynamic_confirmation = matches!(
                    game.mode(),
                    Sf2Mode::Title | Sf2Mode::Briefing | Sf2Mode::PilotSelection
                ) || matches!(
                    game.state().strategic_map.phase,
                    StrategicMapPhase::OpeningOverview | StrategicMapPhase::Tutorial(_)
                ) && game.mode() == Sf2Mode::StrategicMap;
                if dynamic_confirmation {
                    return if t % SF2_DYNAMIC_CONFIRM_CADENCE_TICKS == 0 {
                        pad::B
                    } else {
                        0
                    };
                }
            }
            if t < SF2_MISSION_INPUT_START_TICK {
                return 0;
            }
            if sf2_game.is_some_and(|game| {
                self.sf2_autoplay_pause_after_sorties
                    .is_some_and(|sorties| {
                        game.state().campaign.completed_campaign_visits() >= sorties
                    })
            }) {
                return 0;
            }
            return match sf2_game.map(Sf2Game::mode) {
                Some(Sf2Mode::StrategicMap) => sf2_game
                    .filter(|game| game.state().strategic_map.phase == StrategicMapPhase::Planning)
                    .map(|game| {
                        let map = &game.state().strategic_map;
                        let target = map.recommended_destination;
                        let mut direction = 0;
                        if map.destination.x < target.x {
                            direction |= pad::RIGHT;
                        } else if map.destination.x > target.x {
                            direction |= pad::LEFT;
                        }
                        if map.destination.y < target.y {
                            direction |= pad::DOWN;
                        } else if map.destination.y > target.y {
                            direction |= pad::UP;
                        }
                        if direction == 0 {
                            pad::B
                        } else {
                            direction
                        }
                    })
                    .unwrap_or(0),
                Some(Sf2Mode::Mission) if t % 8 < 4 => pad::B,
                Some(Sf2Mode::GameOver)
                    if sf2_game.is_some_and(|game| {
                        matches!(game.state().game_over.phase, GameOverPhase::Choosing(_))
                    }) =>
                {
                    pad::B
                }
                _ => 0,
            };
        }
        if t <= 400 {
            return if t % 60 == 0 { pad::START } else { 0 };
        }
        if t % 8 < 4 {
            pad::Y // fire lasers while flying
        } else {
            0
        }
    }

    /// C `SfRtl_BeginFrame` (sf_rtl.c:142): latch prev, OR the sources,
    /// edge-detect, THEN increment the frame counter (AutoplayPad reads
    /// the pre-increment count).
    pub fn begin_frame(&mut self, keys: &KeyboardState, sf2_game: Option<&Sf2Game>) {
        self.pad1_prev = self.pad1;
        self.pad1 = Self::read_keyboard(keys) | self.read_gamepads() | self.autoplay_pad(sf2_game);
        self.pad1_new = self.pad1 & !self.pad1_prev;
        self.frame_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C AutoplayPad table check without SDL state: replicate the formula.
    #[test]
    fn autoplay_schedule_matches_sf_rtl() {
        let mut input = Input {
            pad1: 0,
            pad1_new: 0,
            pad1_prev: 0,
            frame_count: 0,
            autoplay: true,
            sf2_autoplay: false,
            sf2_autoplay_pause_after_sorties: None,
            gamepads: Vec::new(),
        };
        let mut presses = Vec::new();
        for t in 0..=420u32 {
            input.frame_count = t;
            if input.autoplay_pad(None) & pad::START != 0 {
                presses.push(t);
            }
        }
        assert_eq!(presses, vec![0, 60, 120, 180, 240, 300, 360]);
        input.frame_count = 401;
        assert_eq!(input.autoplay_pad(None), pad::Y); // 401 % 8 == 1 < 4
        input.frame_count = 404;
        assert_eq!(input.autoplay_pad(None), 0); // 404 % 8 == 4
    }

    #[test]
    fn sf2_autoplay_schedule_reaches_the_first_sortie_inputs() {
        let mut input = Input {
            pad1: 0,
            pad1_new: 0,
            pad1_prev: 0,
            frame_count: 0,
            autoplay: true,
            sf2_autoplay: true,
            sf2_autoplay_pause_after_sorties: None,
            gamepads: Vec::new(),
        };
        for tick in SF2_FRONT_END_START_TICKS {
            input.frame_count = tick;
            assert_eq!(input.autoplay_pad(None), pad::START);
        }
        for tick in SF2_FRONT_END_CONFIRM_TICKS {
            input.frame_count = tick;
            assert_eq!(input.autoplay_pad(None), pad::B);
        }
        input.frame_count = 1_051;
        assert_eq!(input.autoplay_pad(None), 0);
    }

    #[test]
    fn sf2_autoplay_confirms_the_retail_title_after_intro_skips() {
        const TITLE_CONFIRM_TICK: u32 = 190;

        let mut game = Sf2Game::new();
        for tick in 0..=TITLE_CONFIRM_TICK {
            let held = if SF2_FRONT_END_START_TICKS.contains(&tick) {
                sf2_game::Button::Start as u16
            } else {
                0
            };
            game.tick(held).unwrap();
        }
        assert_eq!(game.mode(), Sf2Mode::Title);

        let input = Input {
            pad1: 0,
            pad1_new: 0,
            pad1_prev: 0,
            frame_count: TITLE_CONFIRM_TICK,
            autoplay: true,
            sf2_autoplay: true,
            sf2_autoplay_pause_after_sorties: None,
            gamepads: Vec::new(),
        };
        assert_eq!(input.autoplay_pad(Some(&game)), pad::B);
    }
}
