use std::path::PathBuf;

use sf2_game::{Pilot, PilotCraftClass, ShapeId};

const PILOT_CLASS_TABLE: usize = 0x32462;
const PILOT_MAXIMUM_SHIELD_TABLE: usize = 0x3246E;
const PILOT_CHARGE_THRESHOLD_TABLE: usize = 0x32474;
const PILOT_FLIGHT_CRAFT_CLASS_TABLE: usize = 0x31692;
const PILOT_TRANSFORMATION_CRAFT_TABLE: usize = 0x3F97B;
const PILOT_WALKER_SHAPE_TABLE: usize = 0x3F993;
const SHAPE_HEADER_RECORD_BYTES: u16 = 28;
const TRANSFORMATION_DESCRIPTOR_BYTES: usize = 17;
const FOX_TO_WALKER_DESCRIPTOR: usize = 0x35C50;
const MIYU_FAY_TO_WALKER_DESCRIPTOR: usize = 0x35C61;
const PEPPY_SLIPPY_TO_WALKER_DESCRIPTOR: usize = 0x35C72;
const FOX_TO_FLIGHT_DESCRIPTOR: usize = 0x35C83;
const MIYU_FAY_TO_FLIGHT_DESCRIPTOR: usize = 0x35C94;
const PEPPY_SLIPPY_TO_FLIGHT_DESCRIPTOR: usize = 0x35CA5;
const TRANSFORMATION_FRAME_MODULUS: u8 = 7;
const FIRST_TRANSFORMATION_FRAME: u8 = 1;
const LAST_TRANSFORMATION_FRAME: u8 = 6;
const REVERSE_FRAME_STEP: i8 = -1;
const FORWARD_FRAME_STEP: i8 = 1;
const TO_WALKER_INTER_STAGE_PARAMETER: u16 = 0;
const TO_FLIGHT_INTER_STAGE_PARAMETER: u16 = 5;
const TO_WALKER_TERMINAL_MODE: u8 = 1;
const TO_FLIGHT_TERMINAL_MODE: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransformationStage {
    shape: u16,
    frame_modulus: u8,
    target_frame: u8,
    frame_step: i8,
    initial_frame: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransformationDescriptor {
    first: TransformationStage,
    second: TransformationStage,
    inter_stage_parameter: u16,
    final_shape: u16,
    terminal_mode: u8,
}

fn retail_rom() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn word(rom: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([rom[offset], rom[offset + 1]])
}

fn signed_byte(byte: u8) -> i8 {
    i8::from_ne_bytes([byte])
}

fn transformation_stage(rom: &[u8], offset: usize) -> TransformationStage {
    TransformationStage {
        shape: word(rom, offset),
        frame_modulus: rom[offset + 2],
        target_frame: rom[offset + 3],
        frame_step: signed_byte(rom[offset + 4]),
        initial_frame: rom[offset + 5],
    }
}

fn transformation_descriptor(rom: &[u8], offset: usize) -> TransformationDescriptor {
    assert!(offset + TRANSFORMATION_DESCRIPTOR_BYTES <= rom.len());
    TransformationDescriptor {
        first: transformation_stage(rom, offset),
        second: transformation_stage(rom, offset + 6),
        inter_stage_parameter: word(rom, offset + 12),
        final_shape: word(rom, offset + 14),
        terminal_mode: rom[offset + 16],
    }
}

fn catalog_shape_word(shape: ShapeId) -> u16 {
    shape
        .catalog_entry()
        .expect("native craft shape has a decoded catalog entry")
        .shape_id
}

fn flight_craft_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_FLIGHT_CRAFT,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_FLIGHT_CRAFT,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_FLIGHT_CRAFT,
    }
}

fn flight_side_transition_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_FLIGHT_SIDE_TRANSITION,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_FLIGHT_SIDE_TRANSITION,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_FLIGHT_SIDE_TRANSITION,
    }
}

fn walker_side_transition_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_WALKER_SIDE_TRANSITION,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_WALKER_SIDE_TRANSITION,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_WALKER_SIDE_TRANSITION,
    }
}

fn walker_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_WALKER,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_WALKER,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_WALKER,
    }
}

#[test]
fn native_pilot_profiles_match_the_retail_flight_and_six_entry_tables() {
    let Some(rom) = retail_rom() else {
        return;
    };
    let pilots = Pilot::ALL;

    for (index, pilot) in pilots.into_iter().enumerate() {
        let profile = pilot.craft_profile();
        let (expected_class, flight_class_index) = match profile.class {
            PilotCraftClass::FoxFalco => (1, 0),
            PilotCraftClass::PeppySlippy => (0, 1),
            PilotCraftClass::MiyuFay => (3, 2),
        };
        assert_eq!(rom[PILOT_CLASS_TABLE + index], expected_class);
        assert_eq!(
            rom[PILOT_MAXIMUM_SHIELD_TABLE + index],
            profile.maximum_shield
        );
        assert_eq!(
            rom[PILOT_CHARGE_THRESHOLD_TABLE + index],
            profile.charge_threshold
        );

        let decoded_flight_shape = flight_craft_shape(pilot)
            .catalog_entry()
            .expect("native pilot flight craft has a decoded catalog entry")
            .shape_id;
        assert_eq!(
            word(
                &rom,
                PILOT_FLIGHT_CRAFT_CLASS_TABLE + flight_class_index * 2
            ),
            decoded_flight_shape
        );

        let decoded_transformation_shape = flight_side_transition_shape(pilot)
            .catalog_entry()
            .expect("native pilot transformation craft has a decoded catalog entry")
            .shape_id;
        assert_eq!(
            word(&rom, PILOT_TRANSFORMATION_CRAFT_TABLE + index * 2),
            decoded_transformation_shape
        );
        let decoded_walker_side_transition = walker_side_transition_shape(pilot)
            .catalog_entry()
            .expect("native pilot walker-side transition has a decoded catalog entry")
            .shape_id;
        assert_eq!(
            decoded_walker_side_transition + SHAPE_HEADER_RECORD_BYTES,
            decoded_transformation_shape
        );

        let decoded_walker = walker_shape(pilot)
            .catalog_entry()
            .expect("native pilot walker has a decoded catalog entry")
            .shape_id;
        assert_eq!(
            word(&rom, PILOT_WALKER_SHAPE_TABLE + index * 2),
            decoded_walker
        );
    }
}

#[test]
fn native_craft_transformations_match_the_six_retail_descriptors() {
    let Some(rom) = retail_rom() else {
        return;
    };
    let cases = [
        (
            Pilot::Fox,
            FOX_TO_WALKER_DESCRIPTOR,
            FOX_TO_FLIGHT_DESCRIPTOR,
        ),
        (
            Pilot::Miyu,
            MIYU_FAY_TO_WALKER_DESCRIPTOR,
            MIYU_FAY_TO_FLIGHT_DESCRIPTOR,
        ),
        (
            Pilot::Peppy,
            PEPPY_SLIPPY_TO_WALKER_DESCRIPTOR,
            PEPPY_SLIPPY_TO_FLIGHT_DESCRIPTOR,
        ),
    ];

    for (pilot, to_walker_offset, to_flight_offset) in cases {
        let flight = catalog_shape_word(flight_craft_shape(pilot));
        let flight_side = catalog_shape_word(flight_side_transition_shape(pilot));
        let walker_side = catalog_shape_word(walker_side_transition_shape(pilot));

        assert_eq!(
            transformation_descriptor(&rom, to_walker_offset),
            TransformationDescriptor {
                first: TransformationStage {
                    shape: flight_side,
                    frame_modulus: TRANSFORMATION_FRAME_MODULUS,
                    target_frame: 0,
                    frame_step: REVERSE_FRAME_STEP,
                    initial_frame: LAST_TRANSFORMATION_FRAME,
                },
                second: TransformationStage {
                    shape: walker_side,
                    frame_modulus: TRANSFORMATION_FRAME_MODULUS,
                    target_frame: 0,
                    frame_step: REVERSE_FRAME_STEP,
                    initial_frame: LAST_TRANSFORMATION_FRAME,
                },
                inter_stage_parameter: TO_WALKER_INTER_STAGE_PARAMETER,
                final_shape: walker_side,
                terminal_mode: TO_WALKER_TERMINAL_MODE,
            },
            "retail to-walker descriptor for {pilot:?}"
        );

        assert_eq!(
            transformation_descriptor(&rom, to_flight_offset),
            TransformationDescriptor {
                first: TransformationStage {
                    shape: walker_side,
                    frame_modulus: TRANSFORMATION_FRAME_MODULUS,
                    target_frame: LAST_TRANSFORMATION_FRAME,
                    frame_step: FORWARD_FRAME_STEP,
                    initial_frame: FIRST_TRANSFORMATION_FRAME,
                },
                second: TransformationStage {
                    shape: flight_side,
                    frame_modulus: TRANSFORMATION_FRAME_MODULUS,
                    target_frame: LAST_TRANSFORMATION_FRAME,
                    frame_step: FORWARD_FRAME_STEP,
                    initial_frame: FIRST_TRANSFORMATION_FRAME,
                },
                inter_stage_parameter: TO_FLIGHT_INTER_STAGE_PARAMETER,
                final_shape: flight,
                terminal_mode: TO_FLIGHT_TERMINAL_MODE,
            },
            "retail to-flight descriptor for {pilot:?}"
        );
    }
}
