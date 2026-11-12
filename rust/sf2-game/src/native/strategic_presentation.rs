//! The bank-0B presentation director on the strategic map (record
//! 1FBE..1FC9, `$0B:8C21`), run first in each service frame and in each
//! frame of the map's closing fade. Back from a stage the map fades in
//! (state 0x0F); leaving, it closes an iris on the arrival point (state
//! 0x10). Their window shapes and brightness are presentation; the steps'
//! timing, the dialog hold they release and the hold on the map's frame
//! are not.

use super::presentation_director::{step_list, PresentationDirector, STARTED};
use super::strategic_director::MapDirector;
use super::strategic_screen::{ScreenError, ScreenLinks};

const FADE_IN: u16 = 0x0F;
const CLOSING_IRIS: u16 = 0x10;

#[derive(Debug, Clone, Copy)]
enum Step {
    /// Window and brightness steps (presentation).
    Window,
    /// `$0B:956B`: the dialog hold is released.
    ReleaseDialog,
    /// `$0B:9572`: wait until the dialog hold is taken again.
    AwaitDialog,
    /// `$0B:8E34`: the windows are disabled and the map's frame released.
    ReleaseFrame,
    /// `$0B:8E2C`: the record is cleared.
    End,
    /// A terminal step that holds the record (`$0B:964A`).
    Hold,
}

/// `$0B:954D`: state 0x0F.
const FADE_IN_STEPS: [(u8, Step); 10] = [
    (0x80, Step::Window),
    (0x80, Step::Window),
    (0x1C, Step::Window),
    (0x02, Step::Window),
    (0x80, Step::ReleaseDialog),
    (0x82, Step::AwaitDialog),
    (0x80, Step::ReleaseDialog),
    (0x80, Step::ReleaseFrame),
    (0x02, Step::Window),
    (0x81, Step::End),
];
/// `$0B:963B`: state 0x10; `$0B:9650` when the scene holds the radio
/// (1B88 bit 10).
const CLOSING_STEPS: [(u8, Step); 5] = [
    (0x80, Step::Window),
    (0x80, Step::Window),
    (0x04, Step::Window),
    (0x1C, Step::Window),
    (0x81, Step::Hold),
];
const CLOSING_HELD_STEPS: [(u8, Step); 5] = [
    (0x80, Step::Window),
    (0x80, Step::Window),
    (0x04, Step::Window),
    (0x18, Step::Window),
    (0x81, Step::Hold),
];

/// `$0B:8C17`: drop any request.
pub fn clear(record: &mut PresentationDirector) {
    record.request = 0;
}

/// `$0B:8CC1` (the requests at `$0B:8C52` and `$0B:8C57`): request a
/// state with the HUD shown.
pub fn request(record: &mut PresentationDirector, state: u8) {
    record.hud_shown = true;
    record.request = u16::from(state);
}

/// `$0B:8C21`: one director frame.
pub fn advance(
    record: &mut PresentationDirector,
    director: &mut MapDirector,
    links: &mut ScreenLinks,
    layers: &mut u8,
) -> Result<(), ScreenError> {
    if record.request != 0 {
        if record.request & STARTED == 0 {
            record.state = record.request;
            record.request |= STARTED;
            record.step = 0;
            record.elapsed = 0;
            record.timer = 0;
        }
        let steps: &[(u8, Step)] = match record.state {
            FADE_IN => &FADE_IN_STEPS,
            CLOSING_IRIS if links.scene.scene_events & 0x0400 == 0 => {
                *layers = 0x16;
                &CLOSING_STEPS
            }
            CLOSING_IRIS => &CLOSING_HELD_STEPS,
            other => return Err(ScreenError::Unported(0x0B8C8A + 2 * u32::from(other))),
        };
        step_list(record, steps, |step, record| {
            match step {
                Step::Window | Step::Hold => {}
                Step::ReleaseDialog => director.dialog_hold &= !0x0001,
                Step::AwaitDialog => {
                    if director.dialog_hold & 0x0001 != 0 {
                        record.completion = record.completion.wrapping_sub(1);
                    }
                }
                Step::ReleaseFrame => links.mode &= !0x0020,
                Step::End => {
                    record.request = 0;
                    record.state = 0;
                    record.step = 0;
                }
            }
            Ok::<(), ScreenError>(())
        })?;
    }
    record.frame_counter = record.frame_counter.wrapping_add(1);
    Ok(())
}
