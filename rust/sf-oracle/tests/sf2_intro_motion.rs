//! Execute the retail routines independently of the typed attract kernels.
//! Machine addresses and the dispatcher-return harness stay oracle-only.

use sf2_game::intro_motion::{
    chase_fine_angle, settle_title_view_yaw, AttractCameraAngles, IntroPlayerAnchor,
};
use sf2_game::{Angle, Rotation, Vector3};
use sf_oracle::{call, Entry, SnesBus};

const CURRENT: u16 = 0x03BD;
const SELECTED: u16 = 0x03FC;
const AUX_SLOT: u16 = 0x0140;
const OBJECT_POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const OBJECT_ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const CAPTURE_ROTATION: u32 = 0x7FBFCD;
const CAPTURE_POSITION: u32 = 0x7FBFAB;
const SETTLE_YAW: u32 = 0x07F52B;
const CAMERA_YAW: u32 = 0x7E0353;
const DATA: u32 = 0x7E0000;
const SELECTED_POINTER: u32 = DATA + 0xCF1F;
const SAVED_ROTATION: u32 = DATA + 0x6AB9 + AUX_SLOT as u32;
const SAVED_POSITION: u32 = DATA + 0x6BED + AUX_SLOT as u32;
const SENTINEL: u8 = 0xA5;

fn bus() -> SnesBus {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rom = std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("intro differential tests require the user-owned retail SF2 ROM");
    let mut bus = SnesBus::new(rom.clone());
    for (source, destination, length) in
        [(0x010000, 0x7F0000, 0x7E00), (0x050000, 0x7F7E00, 0x4E00)]
    {
        for offset in 0..length {
            bus.write8(destination + offset as u32, rom[source + offset]);
        }
    }
    // Stop at the handler's common path continuation. The retail handler
    // and its pose-copy helpers are unmodified; no next path command runs.
    bus.write8(0x7FCAE8, 0x6B);
    bus.write8(0x7FCABE, 0x6B);
    bus
}

#[test]
fn every_signed_chase_displacement_matches_retail_with_wrapping_origins() {
    let mut bus = bus();
    for current in [0, 32_768, u16::MAX] {
        for target in u16::MIN..=u16::MAX {
            bus.write16(DATA + 0x3A, current);
            call(
                &mut bus,
                0x7F25A3,
                &Entry {
                    a: target,
                    dbr: 0x7E,
                    ..Default::default()
                },
            );
            assert_eq!(
                chase_fine_angle(current, target),
                bus.read16(DATA + 0x3A),
                "current={current} target={target}"
            );
        }
    }
}

#[test]
fn attract_camera_chase_and_coarse_copy_match_complete_retail_handlers() {
    let mut bus = bus();
    const FIXED_CAMERA: u16 = 0x033F;
    const COMMAND: u32 = DATA + 0x0300;
    bus.write16(DATA + 0xF9, COMMAND as u16);
    bus.write8(DATA + 0xFB, (COMMAND >> 16) as u8);
    bus.write8(COMMAND, 0x43);
    bus.write16(COMMAND + 1, FIXED_CAMERA);
    for angle in u8::MIN..=u8::MAX {
        let source_angles = [angle, angle.wrapping_mul(3), angle.wrapping_add(127)];
        let target = Rotation {
            pitch: Angle::from_units(source_angles[0]),
            yaw: Angle::from_units(source_angles[1]),
            roll: Angle::from_units(source_angles[2]),
        };
        let mut native = AttractCameraAngles {
            pitch: u16::from(angle).wrapping_mul(259),
            yaw: u16::from(angle).wrapping_mul(509),
            roll: u16::from(angle).wrapping_mul(769),
        };
        for (field, value) in OBJECT_ROTATION.into_iter().zip(source_angles) {
            bus.write8(DATA + u32::from(CURRENT + field), value);
        }
        for (field, value) in
            OBJECT_ROTATION
                .into_iter()
                .zip([native.pitch, native.yaw, native.roll])
        {
            bus.write16(DATA + u32::from(FIXED_CAMERA + field), value);
        }
        invoke(&mut bus, 0x7FC069, CURRENT);
        native.chase(target);
        for (field, value) in
            OBJECT_ROTATION
                .into_iter()
                .zip([native.pitch, native.yaw, native.roll])
        {
            assert_eq!(bus.read16(DATA + u32::from(FIXED_CAMERA + field)), value);
        }
        // This handler enters with an eight-bit accumulator: it copies only
        // the high byte of each fine angle, leaving neighboring bytes alone.
        for field in OBJECT_ROTATION {
            bus.write8(DATA + u32::from(CURRENT + field + 1), SENTINEL);
        }
        invoke(&mut bus, 0x7FC0BF, CURRENT);
        let rotation = native.coarse_rotation();
        for (field, value) in
            OBJECT_ROTATION
                .into_iter()
                .zip([rotation.pitch, rotation.yaw, rotation.roll])
        {
            assert_eq!(bus.read8(DATA + u32::from(CURRENT + field)), value.units());
            assert_eq!(bus.read8(DATA + u32::from(CURRENT + field + 1)), SENTINEL);
        }
    }
}

fn invoke(bus: &mut SnesBus, target: u32, current: u16) {
    call(
        bus,
        target,
        &Entry {
            x: current,
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );
}

#[test]
fn every_fine_yaw_matches_the_retail_settling_routine() {
    let mut bus = bus();
    for yaw in u16::MIN..=u16::MAX {
        bus.write16(CAMERA_YAW, yaw);
        invoke(&mut bus, SETTLE_YAW, CURRENT);
        assert_eq!(
            settle_title_view_yaw(yaw),
            bus.read16(CAMERA_YAW),
            "yaw={yaw}"
        );
    }
}

#[test]
fn pose_capture_matches_retail_including_fraction_bytes_and_aliasing() {
    let mut bus = bus();
    for selected in [SELECTED, CURRENT] {
        bus.write16(SELECTED_POINTER, selected);
        bus.write16(DATA + u32::from(selected) + 0x2B, AUX_SLOT);
        for angle in u8::MIN..=u8::MAX {
            let angles = [angle, angle.wrapping_mul(3), angle.wrapping_add(127)];
            let rotation = Rotation {
                pitch: Angle::from_units(angles[0]),
                yaw: Angle::from_units(angles[1]),
                roll: Angle::from_units(angles[2]),
            };
            // Nonzero adjacent bytes expose incorrect odd-word masking.
            for (field, value) in OBJECT_ROTATION.into_iter().zip(angles) {
                bus.write8(DATA + u32::from(CURRENT + field - 1), SENTINEL);
                bus.write8(DATA + u32::from(CURRENT + field), value);
                bus.write8(DATA + u32::from(CURRENT + field + 1), SENTINEL);
            }
            for offset in 0..6 {
                bus.write8(SAVED_ROTATION + offset, SENTINEL);
            }
            invoke(&mut bus, CAPTURE_ROTATION, CURRENT);
            let mut native = IntroPlayerAnchor::default();
            native.capture_rotation(rotation);
            for (field, value) in OBJECT_ROTATION.into_iter().zip(angles) {
                assert_eq!(bus.read8(DATA + u32::from(selected + field)), value);
            }
            assert_eq!(bus.read16(SAVED_ROTATION), native.retained_rotation.pitch);
            assert_eq!(bus.read16(SAVED_ROTATION + 2), native.retained_rotation.yaw);
            assert_eq!(
                bus.read16(SAVED_ROTATION + 4),
                u16::from(native.retained_rotation.roll.units())
            );
        }
        for value in [i16::MIN, -16_385, -1, 0, 1, 16_384, i16::MAX] {
            let position = Vector3 {
                x: value,
                y: value.wrapping_neg(),
                z: value.wrapping_add(1),
            };
            for (field, value) in OBJECT_POSITION
                .into_iter()
                .zip([position.x, position.y, position.z])
            {
                bus.write16(DATA + u32::from(CURRENT + field), value as u16);
            }
            invoke(&mut bus, CAPTURE_POSITION, CURRENT);
            let mut native = IntroPlayerAnchor::default();
            native.capture_position(position);
            for (axis, (field, value)) in OBJECT_POSITION
                .into_iter()
                .zip([
                    native.retained_position.x,
                    native.retained_position.y,
                    native.retained_position.z,
                ])
                .enumerate()
            {
                assert_eq!(bus.read16(DATA + u32::from(selected + field)) as i16, value);
                assert_eq!(bus.read16(SAVED_POSITION + axis as u32 * 2) as i16, value);
            }
        }
    }
}
