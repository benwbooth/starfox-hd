//! Character-map screen selection (ROM `setcharmap*_l`).
//!
//! On the SNES these routines write VRAM tilemaps for the 3D playfield /
//! planet-select / fox-continue screens (`SetCharMapgame_l`,
//! `SetCharMapplan_l`, `setcharmapfox_l`). HD has no VRAM; the equivalent is
//! selecting which UI layout is active. `SETCHARMAPFROMMAP_L` already returns
//! true from the map callback (game.rs) — it just calls `setcharmapgame_l`.

/// Which SNES character-map layout the ROM would have uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharMapScreen {
    /// Idle / unset.
    #[default]
    None,
    /// ROM `SetCharMapgame_l` (MAIN.ASM:1769) — in-game 3D viewport tilemap.
    Game,
    /// ROM `SetCharMapplan_l` (PLANETS.ASM:2936) — planet-select tilemap.
    Plan,
    /// ROM `setcharmapfox_l` (CONTINUE.ASM:459) — fox continue / end-seq.
    Fox,
}

/// Active character-map screen (HD stand-in for the last `setcharmap*_l`).
#[derive(Debug, Clone, Default)]
pub struct CharMap {
    pub screen: CharMapScreen,
}

impl CharMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// ROM `SetCharMapgame_l` / `setcharmapfrommap_l` body.
    pub fn set_game(&mut self) {
        self.screen = CharMapScreen::Game;
    }

    /// ROM `SetCharMapplan_l`.
    pub fn set_plan(&mut self) {
        self.screen = CharMapScreen::Plan;
    }

    /// ROM `setcharmapfox_l`.
    pub fn set_fox(&mut self) {
        self.screen = CharMapScreen::Fox;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setcharmap_screens() {
        let mut cm = CharMap::new();
        assert_eq!(cm.screen, CharMapScreen::None);
        cm.set_game();
        assert_eq!(cm.screen, CharMapScreen::Game);
        cm.set_plan();
        assert_eq!(cm.screen, CharMapScreen::Plan);
        cm.set_fox();
        assert_eq!(cm.screen, CharMapScreen::Fox);
    }
}
