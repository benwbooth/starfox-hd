//! Unmodified clamp/easing tail between point projection and lock retention.

use super::*;
use sf2_game::player_target_lock::{ReticlePositionError, TargetReticle};

const ENTRY: u32 = 0x07A471;
const STOP: u32 = 0x07A505;

fn compare(source: &mut Source, projected: [i16; 2], previous: [u8; 2]) {
    source.bus.write16(WRAM + 0x79, projected[0] as u16);
    source.bus.write16(WRAM + 0x7B, projected[1] as u16);
    source.bus.write8(WRAM + 0x1E30, previous[0]);
    source.bus.write8(WRAM + 0x1E31, previous[1]);
    source.run(ENTRY, Some(STOP), 0, OWNER, true);
    let mut reticle = TargetReticle {
        horizontal: Some(previous[0]),
        vertical: Some(previous[1]),
    };
    reticle.track_projected(projected).unwrap();
    assert_eq!(
        reticle.horizontal,
        Some(source.bus.read8(WRAM + 0x1E30)),
        "{projected:?} {previous:?}"
    );
    assert_eq!(
        reticle.vertical,
        Some(source.bus.read8(WRAM + 0x1E31)),
        "{projected:?} {previous:?}"
    );
    for &(address, _) in source.writes.as_ref().unwrap() {
        assert!(
            address < 0x033F || [WRAM + 0x1E30, WRAM + 0x1E31].contains(&address),
            "unexpected persistent write {address:06X}"
        );
    }
}

#[test]
fn reticle_tracking_matches_original_every_projected_word_and_byte_easing_pair() {
    let mut source = Source::new(&rom(), 0xA7);
    source.writes = Some(Vec::new());
    for axis in 0..2 {
        for bits in 0..=u16::MAX {
            let mut projected = [104, 104];
            projected[axis] = bits as i16;
            compare(
                &mut source,
                projected,
                [bits as u8, (bits as u8).rotate_left(3)],
            );
        }
        for projected_byte in 0..=u8::MAX {
            for previous_byte in 0..=u8::MAX {
                let mut projected = [104, 104];
                projected[axis] = i16::from(projected_byte);
                let mut previous = [128, 128];
                previous[axis] = previous_byte;
                compare(&mut source, projected, previous);
            }
        }
    }
}

#[test]
fn reticle_tracking_missing_axis_preserves_original_completed_horizontal_prefix() {
    let mut source = Source::new(&rom(), 0xA7);
    for vertical in [false, true] {
        for previous in 0..=u8::MAX {
            let projected = [17, 31];
            source.bus.write16(WRAM + 0x79, projected[0] as u16);
            source.bus.write16(WRAM + 0x7B, projected[1] as u16);
            source.bus.write8(WRAM + 0x1E30, previous);
            source.bus.write8(WRAM + 0x1E31, !previous);
            source.run(
                ENTRY,
                Some(if vertical { 0x07A4DC } else { 0x07A4AA }),
                0,
                OWNER,
                true,
            );
            let mut reticle = TargetReticle {
                horizontal: vertical.then_some(previous),
                vertical: (!vertical).then_some(!previous),
            };
            assert_eq!(
                reticle.track_projected(projected),
                Err(if vertical {
                    ReticlePositionError::MissingVertical
                } else {
                    ReticlePositionError::MissingHorizontal
                })
            );
            if vertical {
                assert_eq!(reticle.horizontal, Some(source.bus.read8(WRAM + 0x1E30)));
                assert_eq!(reticle.vertical, None);
            } else {
                assert_eq!(reticle.horizontal, None);
                assert_eq!(reticle.vertical, Some(source.bus.read8(WRAM + 0x1E31)));
            }
        }
    }
}
