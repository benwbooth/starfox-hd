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

pub struct Input {
    /// C `g_pad1` / `g_pad1_new` / `g_pad1_prev`.
    pub pad1: u16,
    pub pad1_new: u16,
    pub pad1_prev: u16,
    /// C `g_frame_count` (ticks, not render frames).
    pub frame_count: u32,
    /// SF_AUTOPLAY=1 (C AutoplayPad enable latch).
    autoplay: bool,
    /// All open gamepads, keyed by joystick instance id.
    pub gamepads: Vec<Gamepad>,
}

impl Input {
    pub fn new() -> Self {
        let autoplay = std::env::var("SF_AUTOPLAY")
            .map(|v| v.starts_with('1'))
            .unwrap_or(false);
        Input {
            pad1: 0,
            pad1_new: 0,
            pad1_prev: 0,
            frame_count: 0,
            autoplay,
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

    pub fn add_gamepad(&mut self, subsystem: &sdl3::GamepadSubsystem, id: sdl3::joystick::JoystickId) {
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
    /// In Star Fox that means: Y(left)=laser, A(right)=nova bomb, B(bottom)=
    /// brake, X(top)=boost.
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
                    (Button::South, "South"), (Button::East, "East"),
                    (Button::West, "West"), (Button::North, "North"),
                    (Button::Start, "Start"), (Button::Back, "Back"),
                    (Button::Guide, "Guide"), (Button::LeftShoulder, "L"),
                    (Button::RightShoulder, "R"), (Button::LeftStick, "LS"),
                    (Button::RightStick, "RS"), (Button::DPadUp, "Up"),
                    (Button::DPadDown, "Down"), (Button::DPadLeft, "Left"),
                    (Button::DPadRight, "Right"),
                ] {
                    if gp.button(b) {
                        let _ = write!(s, "{name} ");
                    }
                }
                for (a, name) in [
                    (Axis::LeftX, "LX"), (Axis::LeftY, "LY"),
                    (Axis::RightX, "RX"), (Axis::RightY, "RY"),
                    (Axis::TriggerLeft, "LT"), (Axis::TriggerRight, "RT"),
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
            let mut lx = gp.axis(Axis::LeftX);
            let ly = gp.axis(Axis::LeftY);
            // The Steam Controller has no proper SDL gamepad mapping, so SDL
            // reports its LeftX axis inverted (stick-left -> positive -> RIGHT).
            // The game logic itself is correct (proven by tests/steering.rs);
            // this is purely the unmapped-axis direction. Flip it for that
            // controller so stick-left steers left. LeftY reads correctly.
            if gp.name().as_deref() == Some("Steam Controller") {
                lx = lx.saturating_neg();
            }
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

    /// C `AutoplayPad` (sf_rtl.c:123) — EXACT replica: Start taps every
    /// 60 ticks through tick 400, then 4-on/4-off fire pulses.
    fn autoplay_pad(&self) -> u16 {
        if !self.autoplay {
            return 0;
        }
        let t = self.frame_count;
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
    pub fn begin_frame(&mut self, keys: &KeyboardState) {
        self.pad1_prev = self.pad1;
        self.pad1 = Self::read_keyboard(keys) | self.read_gamepads() | self.autoplay_pad();
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
            gamepads: Vec::new(),
        };
        let mut presses = Vec::new();
        for t in 0..=420u32 {
            input.frame_count = t;
            if input.autoplay_pad() & pad::START != 0 {
                presses.push(t);
            }
        }
        assert_eq!(presses, vec![0, 60, 120, 180, 240, 300, 360]);
        input.frame_count = 401;
        assert_eq!(input.autoplay_pad(), pad::Y); // 401 % 8 == 1 < 4
        input.frame_count = 404;
        assert_eq!(input.autoplay_pad(), 0); // 404 % 8 == 4
    }
}
