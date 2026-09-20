//! Authored radio requests, before the separate presentation service runs.
//! `$0A:CF04` replaces the pending message and chooses its initial panel
//! placement. It does not render text, open a portrait, or enqueue audio.

const MESSAGE_NUMBER_BASE: u8 = 1;
const LOWER_PANEL_Y: u16 = 151;
const COMPACT_LOWER_PANEL_Y: u16 = 139;
const UPPER_PANEL_Y: u16 = 35;
const TRACKED_Y_THRESHOLD: u8 = 146;
const BYTE_SIGN_BOUNDARY: u8 = 128;

/// Zero-based authored message identity, not a source address or text pointer.
/// The source accepts all byte values: authored number zero wraps to index 255.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MessageIndex(u8);

impl MessageIndex {
    pub const fn from_authored_number(number: u8) -> Self {
        Self(number.wrapping_sub(MESSAGE_NUMBER_BASE))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RadioRequest {
    pub message: MessageIndex,
    pub pending: bool,
    /// Full vertical coordinate: requesting resets its high byte as well.
    pub panel_y: u16,
    pub top_placement: bool,
}

/// Authored deferred message word ($D790). The guidance controller imports
/// it, adds the published wingmate identity, requests a message and clears
/// it. This is separate from the already formatted presentation request.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeferredMessage {
    pub number: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredMessageCommand {
    CopyTo(super::path_fields::WordField),
    Assign(super::path_fields::WordOperand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioLayout {
    /// The live nonzero compact-layout flag read by the source radio service.
    pub compact_panel: bool,
    /// Smoothed projected tracking-marker coordinate from `$07:A439..A502`.
    /// It is not the selected actor's world-space height.
    pub tracked_screen_y: u8,
}

pub struct PathRadio<'a> {
    pub request: &'a mut RadioRequest,
    pub layout: RadioLayout,
}

impl PathRadio<'_> {
    pub fn request_message(&mut self, number: u8) {
        // The source tests the sign of byte subtraction, NOT unsigned >=.
        // Consequently 0..17 wrap into the upper-placement interval too.
        let top = self
            .layout
            .tracked_screen_y
            .wrapping_sub(TRACKED_Y_THRESHOLD)
            < BYTE_SIGN_BOUNDARY;
        *self.request = RadioRequest {
            message: MessageIndex::from_authored_number(number),
            pending: true,
            panel_y: if top {
                UPPER_PANEL_Y
            } else if self.layout.compact_panel {
                COMPACT_LOWER_PANEL_Y
            } else {
                LOWER_PANEL_Y
            },
            top_placement: top,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_message_numbers_and_layout_bytes_replace_the_request_with_source_widths() {
        for compact_panel in [false, true] {
            for tracked_screen_y in 0..=u8::MAX {
                for number in 0..=u8::MAX {
                    let mut request = RadioRequest {
                        message: MessageIndex(53),
                        pending: number % 2 == 0,
                        panel_y: 0xDEAD,
                        top_placement: true,
                    };
                    PathRadio {
                        request: &mut request,
                        layout: RadioLayout {
                            compact_panel,
                            tracked_screen_y,
                        },
                    }
                    .request_message(number);
                    let top = !(18..146).contains(&tracked_screen_y);
                    assert_eq!(
                        request,
                        RadioRequest {
                            message: MessageIndex(if number == 0 { 255 } else { number - 1 }),
                            pending: true,
                            panel_y: if top {
                                35
                            } else if compact_panel {
                                139
                            } else {
                                151
                            },
                            top_placement: top,
                        }
                    );
                    assert_eq!(request.message.index(), (usize::from(number) + 255) % 256);
                }
            }
        }
    }
}
