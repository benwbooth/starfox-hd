//! Background horizontal scroll publication (`$03:B0C3..B0EE`), called from
//! the mission frame loop before the strategy epoch.
//!
//! The source widens the fixed view's full yaw word, shifts it left twice and
//! keeps the middle word, which is the yaw divided by 64. It then adds the
//! authored background base (1E4E) and stores the sum in the scroll word 193C.
//! Both additions wrap at sixteen bits.

use super::path_runtime::PathRuntime;
use super::scene_path_world::ScenePathWorld;
use super::view_transition::FixedViewAngles;
use super::ObjectStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundScrollError {
    MissingFixedView,
    MissingViewActor,
    /// 1E4E is cleared by scene resets the native owner has not ported yet;
    /// an unset base is a fault, never an assumed zero.
    MissingHorizontalBase,
}

const YAW_TO_SCROLL_SHIFT: u32 = 6;

/// Scroll word for a full view yaw word and the authored base.
pub const fn scroll_x(view_yaw: u16, horizontal_base: i16) -> u16 {
    (view_yaw >> YAW_TO_SCROLL_SHIFT).wrapping_add(horizontal_base as u16)
}

pub fn publish(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    runtime: &mut PathRuntime,
) -> Result<u16, BackgroundScrollError> {
    let view = world.fixed_players[0].ok_or(BackgroundScrollError::MissingFixedView)?;
    let camera = objects
        .get(view)
        .ok_or(BackgroundScrollError::MissingViewActor)?;
    let base = runtime
        .background_horizontal
        .ok_or(BackgroundScrollError::MissingHorizontalBase)?;
    let scroll = scroll_x(FixedViewAngles::capture(camera).yaw, base);
    runtime.background_scroll_x = Some(scroll);
    Ok(scroll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_divides_the_full_yaw_word_by_sixty_four_and_wraps_the_base() {
        assert_eq!(scroll_x(0, 0), 0);
        assert_eq!(scroll_x(0x0040, 0), 1);
        assert_eq!(scroll_x(0xFFFF, 0), 0x03FF);
        assert_eq!(scroll_x(0xFFFF, -0x03FF), 0);
        assert_eq!(scroll_x(0x8000, i16::MAX), 0x0200u16.wrapping_add(0x7FFF));
    }
}
