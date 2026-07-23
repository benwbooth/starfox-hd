/// One logical SNES controller button. The discriminants are protocol masks,
/// so bit-oriented notation is intentional here.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    B = 1 << 15,
    Y = 1 << 14,
    Select = 1 << 13,
    Start = 1 << 12,
    Up = 1 << 11,
    Down = 1 << 10,
    Left = 1 << 9,
    Right = 1 << 8,
    A = 1 << 7,
    X = 1 << 6,
    LeftShoulder = 1 << 5,
    RightShoulder = 1 << 4,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Buttons(u16);

impl Buttons {
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, button: Button) -> bool {
        self.0 & button as u16 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputState {
    pub held: Buttons,
    pub pressed: Buttons,
}

impl InputState {
    pub fn sample(&mut self, held: Buttons) {
        self.pressed = Buttons::from_bits(held.bits() & !self.held.bits());
        self.held = held;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressed_buttons_are_edges_not_repeats() {
        let mut input = InputState::default();
        input.sample(Buttons::from_bits(Button::Start as u16));
        assert!(input.pressed.contains(Button::Start));
        input.sample(Buttons::from_bits(Button::Start as u16));
        assert!(!input.pressed.contains(Button::Start));
        input.sample(Buttons::default());
        input.sample(Buttons::from_bits(Button::Start as u16));
        assert!(input.pressed.contains(Button::Start));
    }
}
