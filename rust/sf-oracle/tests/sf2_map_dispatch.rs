//! Original-code proof for scene-map dispatch and frame-owner gating. No
//! observed gameplay schedule supplies a delay or a phase transition here.

use sf2_data::map::{MapAddress, SpawnRecord, EXTERNAL_PHASE_GATES, MAP_COMMANDS};
use sf2_map::{MapVm, RunStop, Sf2MapHost};
use sf_oracle::{call, Entry, SnesBus};

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn file(address: u32) -> usize {
    ((address >> 16) as usize & 0x7F) * 0x8000 + (address as usize & 0x7FFF)
}

fn dispatch(source: &mut SnesBus, cursor: MapAddress) {
    source.write8(0x192E, cursor.bank);
    source.write16(0x1657, cursor.address.wrapping_sub(0x8000));
    let result = call(
        source,
        0x038FC9,
        &Entry {
            x: cursor.address.wrapping_sub(0x8000),
            p: 0x20,
            ..Default::default()
        },
    );
    assert!(result.returned, "dispatcher did not return at {cursor:?}");
}

/// Only control predicates are expected in these scoped invocations. An
/// accidentally reached scene effect fails the test instead of succeeding
/// through a default service implementation.
#[derive(Default)]
struct ControlHost {
    mode: u8,
    external_flags: u16,
}

impl Sf2MapHost for ControlHost {
    type Error = std::convert::Infallible;

    fn mode(&self) -> u8 {
        self.mode
    }
    fn read_long_word(&self, address: u32) -> Result<u16, Self::Error> {
        assert_eq!(address, 0x7EE087);
        Ok(self.external_flags)
    }
    fn display_ready(&self) -> bool {
        panic!("unexpected display query")
    }
    fn load_table_idle(&self) -> bool {
        panic!("unexpected load query")
    }
    fn request_stage_load(&mut self, _: u16) -> Result<(), Self::Error> {
        panic!("unexpected load")
    }
    fn set_current_object_byte(&mut self, _: u16, _: u8) -> Result<(), Self::Error> {
        panic!("unexpected object write")
    }
    fn set_f3(&mut self, _: i8) -> Result<(), Self::Error> {
        panic!("unexpected fade")
    }
    fn write_long_byte(&mut self, _: u32, _: u8) -> Result<(), Self::Error> {
        panic!("unexpected byte write")
    }
    fn write_long_word(&mut self, _: u32, _: u16) -> Result<(), Self::Error> {
        panic!("unexpected word write")
    }
    fn read_long_byte(&self, _: u32) -> Result<u8, Self::Error> {
        panic!("unexpected byte read")
    }
    fn request_post_load(&mut self) -> Result<(), Self::Error> {
        panic!("unexpected post-load")
    }
    fn call_65816(&mut self, _: u32, _: Option<u8>) -> Result<(), Self::Error> {
        panic!("unexpected scene callback")
    }
    fn spawn_object(&mut self, _: SpawnRecord) -> Result<(), Self::Error> {
        panic!("unexpected spawn")
    }
    fn set_current_object_path(&mut self, _: u16) -> Result<(), Self::Error> {
        panic!("unexpected path install")
    }
    fn spawn_aux_object(
        &mut self,
        _: u16,
        _: u16,
        _: u16,
        _: u8,
        _: u16,
        _: u16,
    ) -> Result<(), Self::Error> {
        panic!("unexpected auxiliary spawn")
    }
    fn configure_slot(&mut self, _: u8, _: bool, _: [u8; 7]) -> Result<(), Self::Error> {
        panic!("unexpected slot install")
    }
    fn set_gsu_word_01bc(&mut self, _: u16) -> Result<(), Self::Error> {
        panic!("unexpected renderer setup")
    }
}

#[test]
fn all_authored_phase_loops_dispatch_again_without_consuming_the_marker() {
    let mut source = SnesBus::new(rom());
    let mut host = ControlHost::default();
    assert_eq!(EXTERNAL_PHASE_GATES.len(), 237);
    for gate in EXTERNAL_PHASE_GATES {
        for previous in [0, 1, 5000, u16::MAX] {
            let mut native = MapVm::new(gate.hold);
            native.set_counter(previous);
            source.write16(0x1655, previous);
            for visit in 0..4 {
                dispatch(&mut source, native.cursor());
                let result = native.run(&mut host, 3).unwrap();
                assert_eq!(result.stop, RunStop::CounterSet(5000));
                assert_eq!(result.commands_executed, if visit == 0 { 1 } else { 2 });
                assert_eq!(native.cursor(), gate.parked);
                assert_eq!(source.read16(0x1657), native.cursor().address - 0x8000);
                assert_eq!(source.read16(0x1655), native.counter());
                assert_eq!(native.counter(), 5000);
            }
        }
    }
}

#[test]
fn every_authored_mode_branch_matches_the_original_for_all_mode_bytes() {
    let original = rom();
    let mut count = 0;
    for command in MAP_COMMANDS
        .iter()
        .filter(|command| matches!(command.opcode, 0x9E | 0xA2 | 0xA4))
    {
        let operand = if command.opcode == 0x9E { 2 } else { 1 };
        let target = 0x8000 + u16::from_le_bytes([command.raw[operand], command.raw[operand + 1]]);
        let next = command.address.address + u16::from(command.size);
        // Replace only the first DATA opcode at either successor by STOP.
        // The dispatcher and branch handler remain the unmodified source.
        let mut bounded = original.clone();
        for successor in [target, next] {
            assert_ne!(successor, command.address.address);
            bounded[file((u32::from(command.address.bank) << 16) | u32::from(successor))] = 2;
        }
        let mut source = SnesBus::new(bounded);
        for mode in 0..=u8::MAX {
            let external_flags = if mode & 1 == 0 { 0xFBFF } else { 0xFFFF };
            source.write16(0x1BA5, 0xA500 | u16::from(mode));
            source.write16(0x7EE087, external_flags);
            source.write16(0x1655, 0xCAFE);
            dispatch(&mut source, command.address);
            let mut native = MapVm::new(command.address);
            native.set_counter(0xCAFE);
            let mut host = ControlHost {
                mode,
                external_flags,
            };
            assert_eq!(
                native.run(&mut host, 1).unwrap().stop,
                RunStop::BudgetExhausted
            );
            assert_eq!(
                source.read16(0x1657),
                native.cursor().address - 0x8000,
                "{command:?} mode={mode}"
            );
            assert_eq!(source.read16(0x1655), native.counter());
            count += 1;
        }
    }
    assert_eq!(count, (126 + 7 + 1) * 256);
}

#[test]
fn delay_zero_preserves_the_marker_and_nonzero_values_yield_only_this_visit() {
    let mut source = SnesBus::new(rom());
    // A caller-authored data stream in WRAM exercises boundary operands that
    // do not occur in the retail catalog. The executing code is unchanged.
    let cursor = MapAddress {
        bank: 0x7E,
        address: 0xA000,
    };
    source.write8(0x7EA000, 0x12);
    source.write8(0x7EA003, 0x02);
    for value in 0..=u16::MAX {
        source.write16(0x7EA001, value);
        source.write16(0x1655, 0xCAFE);
        dispatch(&mut source, cursor);
        assert_eq!(source.read16(0x1657), 0x2003);
        let expected = if value == 0 { 0xCAFE } else { value };
        assert_eq!(source.read16(0x1655), expected, "operand={value}");
        // STOP executes on the next visit even for the largest marker.
        dispatch(
            &mut source,
            MapAddress {
                address: 0xA003,
                ..cursor
            },
        );
        assert_eq!(source.read16(0x1655), expected);
        assert_eq!(source.read16(0x1657), 0x2003);
    }
}

#[test]
fn original_frame_owner_does_not_turn_map_yields_into_countdowns() {
    let mut isolated = rom();
    // Bound unrelated scene/render/audio services at their far-call entries.
    // Clearing the renderer-work byte completes those excluded async jobs.
    // The entire frame-owner code and map dispatcher execute unmodified.
    let services = [
        0x7F148D, 0x7F32A1, 0x7F7A1E, 0x03D87D, 0x7F1118, 0x04FCC8, 0x07950E, 0x07BD46, 0x07EA67,
        0x03B0C3, 0x07AA8C, 0x07A326, 0x7F539C, 0x048301, 0x0DD5FA, 0x7F34E7, 0x7F354A, 0x7F11BC,
        0x7F7B33, 0x7F7980, 0x07A337, 0x7F79F8, 0x7F7918, 0x7F11C6, 0x04A3ED, 0x03B0EF, 0x0ACD66,
        0x0B8010, 0x0AD049, 0x03E78D,
    ];
    for target in services.into_iter().filter(|target| target >> 16 != 0x7F) {
        let offset = file(target);
        isolated[offset..offset + 3].copy_from_slice(&[0x64, 0x00, 0x6B]);
    }
    let mut source = SnesBus::new(isolated);
    for target in services.into_iter().filter(|target| target >> 16 == 0x7F) {
        for (delta, byte) in [0x64, 0x00, 0x6B].into_iter().enumerate() {
            source.write8(target + delta as u32, byte);
        }
    }
    source.write8(0x7EA000, 0x12);
    source.write16(0x7EA001, 5000);
    source.write8(0x7EA003, 2);
    for flags in 0..=u8::MAX {
        for marker in [0, 1, 5000, u16::MAX] {
            source.write8(0x1AA6, flags);
            source.write8(0x192E, 0x7E);
            source.write16(0x1657, 0x2000);
            source.write16(0x1655, marker);
            source.write8(0, 0);
            let result = call(
                &mut source,
                0x038004,
                &Entry {
                    p: 0x20,
                    ..Default::default()
                },
            );
            assert!(result.returned, "flags={flags:02X} marker={marker}");
            // Only the alternate-view + map-suppression conjunction skips it.
            let skipped = flags & 0x21 == 0x21;
            assert_eq!(source.read16(0x1657), if skipped { 0x2000 } else { 0x2003 });
            assert_eq!(source.read16(0x1655), if skipped { marker } else { 5000 });
        }
    }
}
