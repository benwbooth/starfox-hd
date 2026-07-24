//! Player camera modes shared by gameplay, map setup, and presentation.
//!
//! The original game stores the current mode and the exclusive upper bound of
//! the selectable cycle as adjacent one-byte globals (`splayerflymode` and
//! `splayerflymodeopt`, GILESALC.INC).  The port keeps the same two pieces of
//! domain state as enums instead of exposing their numeric encoding to
//! gameplay code.

/// Current player camera mode (`spfm_*`, GILESALC.INC:115-121).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerViewMode {
    /// Ordinary exterior chase camera.
    #[default]
    Exterior = 0,
    /// Closer exterior chase camera.
    CloseExterior = 1,
    /// Authored transition from the exterior camera into the cockpit.
    EnteringCockpit = 2,
    /// First-person cockpit camera.
    Cockpit = 3,
    /// Authored transition from the cockpit back to the exterior camera.
    LeavingCockpit = 4,
}

/// Modes made available by the active background's `pstrat` declaration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerViewOptions {
    /// Cleared boot state before a background supplies its declaration.
    #[default]
    Unconfigured = 0,
    /// Exterior and close-exterior views (`spfmo_AB`).
    ExteriorViews = 2,
    /// Exterior, close-exterior, and cockpit views (`spfmo_ABC`).
    ExteriorAndCockpit = 5,
}

impl PlayerViewMode {
    /// Advance exactly as the source's increment-and-exclusive-bound cycle.
    /// Transition states normally replace themselves before another input can
    /// be accepted, but spelling out every state keeps recovery deterministic.
    pub const fn next(self, options: PlayerViewOptions) -> Self {
        match options {
            PlayerViewOptions::Unconfigured => Self::Exterior,
            PlayerViewOptions::ExteriorViews => match self {
                Self::Exterior => Self::CloseExterior,
                _ => Self::Exterior,
            },
            PlayerViewOptions::ExteriorAndCockpit => match self {
                Self::Exterior => Self::CloseExterior,
                Self::CloseExterior => Self::EnteringCockpit,
                Self::EnteringCockpit => Self::Cockpit,
                Self::Cockpit => Self::LeavingCockpit,
                Self::LeavingCockpit => Self::Exterior,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerViewMode as Mode, PlayerViewOptions as Options};

    #[test]
    fn exterior_cycle_wraps_after_close_view() {
        assert_eq!(
            Mode::Exterior.next(Options::ExteriorViews),
            Mode::CloseExterior
        );
        assert_eq!(
            Mode::CloseExterior.next(Options::ExteriorViews),
            Mode::Exterior
        );
    }

    #[test]
    fn cockpit_cycle_enters_and_leaves_through_authored_transitions() {
        assert_eq!(
            Mode::CloseExterior.next(Options::ExteriorAndCockpit),
            Mode::EnteringCockpit
        );
        assert_eq!(
            Mode::Cockpit.next(Options::ExteriorAndCockpit),
            Mode::LeavingCockpit
        );
        assert_eq!(
            Mode::LeavingCockpit.next(Options::ExteriorAndCockpit),
            Mode::Exterior
        );
    }

    #[test]
    fn typed_encodings_match_the_retail_allocation_record() {
        let source = include_str!("../../../reference/ultrastarfox/SF/INC/GILESALC.INC");
        for declaration in [
            "spfm_norm\t\tequ\t0",
            "spfm_close\t\tequ\t1",
            "spfm_toinside\t\tequ\t2",
            "spfm_inside\t\tequ\t3",
            "spfm_tonorm\t\tequ\t4",
            "spfmo_AB\tequ\tspfm_toinside",
            "spfmo_ABC\tequ\tspfm_maxmode",
        ] {
            assert!(
                source.contains(declaration),
                "missing source declaration: {declaration}"
            );
        }

        assert_eq!(Mode::Exterior as u8, 0);
        assert_eq!(Mode::CloseExterior as u8, 1);
        assert_eq!(Mode::EnteringCockpit as u8, 2);
        assert_eq!(Mode::Cockpit as u8, 3);
        assert_eq!(Mode::LeavingCockpit as u8, 4);
        assert_eq!(Options::ExteriorViews as u8, 2);
        assert_eq!(Options::ExteriorAndCockpit as u8, 5);
    }
}
