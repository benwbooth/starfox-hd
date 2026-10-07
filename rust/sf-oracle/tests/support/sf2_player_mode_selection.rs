//! Whole original Select arbitration, pending-mode table and strategy entry.
//! No transform completion or original output is injected into native state.

use super::surface_particle_tests::{Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_mode_selection::{self, ModeRequest};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::{Button, Buttons, InputState, Vector3};

const REQUESTS: [ModeRequest; 3] = [
    ModeRequest::FreeFlight,
    ModeRequest::RetainedPitch,
    ModeRequest::Walker,
];

struct Fixture {
    native: Native,
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        let mut native = Native::new(source, 1, 0, 0, 0);
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut PathRuntime::default(),
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 19,
                score: Default::default(),
            },
        )
        .unwrap();
        native.world.player_carry_mode = Some(0);
        native.world.processed_player_input = Some(Default::default());
        Self { native }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn seed_retained(&mut self, source: &mut Source) {
        let state = self.records().mode_selection.unwrap();
        source.bus.write8(WRAM + SLOT + 0x6AA1, state.requested);
        source
            .bus
            .write8(WRAM + SLOT + 0x6BEC, state.transition_control);
        source.bus.write8(
            WRAM + u32::from(OWNER) + 0x1CC7,
            self.native
                .objects
                .get(self.native.owner)
                .unwrap()
                .base
                .behavior_phase,
        );
    }
    fn step(&mut self, source: &mut Source, request: ModeRequest) -> bool {
        let state = self.records().mode_selection.unwrap();
        source.bus.write8(
            WRAM + SLOT + 0x6B9B,
            0xAD | u8::from(state.request_inhibited) * 0x40,
        );
        source
            .bus
            .write8(WRAM + SLOT + 0x6B64, state.surface_control);
        source
            .bus
            .write8(WRAM + SLOT + 0x6AA0, self.records().auxiliary.unwrap().mode);
        let return_position = self.records().boundary.unwrap().return_position;
        for (offset, value) in [
            (0x6BED, return_position.x),
            (0x6BEF, return_position.y),
            (0x6BF1, return_position.z),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value as u16);
        }
        source.bus.write8(WRAM + SLOT + 0x6AA2, 0xB2);
        source.bus.write8(WRAM + u32::from(OWNER) + 0x1CC8, 0x9F);
        source.bus.write8(
            WRAM + u32::from(OWNER) + 0x21,
            0xDA | u8::from(
                self.native
                    .objects
                    .get(self.native.owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .carry_selected_player,
            ) * 0x20,
        );
        source
            .bus
            .write8(0x1E13, self.native.world.player_carry_mode.unwrap());
        let input = self.native.world.processed_player_input.unwrap();
        source.bus.write16(0x1936, input.pressed.bits());
        source.bus.write16(0x1938, input.held.bits());
        source.bus.write8(2, request.selector());
        let mut before = *self.records();
        source.run(0x0698CD, None, 0, OWNER, true);
        let actual = player_mode_selection::advance(
            &mut self.native.objects,
            &mut self.native.world,
            self.native.owner,
            request,
        )
        .unwrap();
        assert_eq!(actual, source.last_carry);
        assert_eq!(
            self.native
                .objects
                .get(self.native.owner)
                .unwrap()
                .base
                .behavior_phase,
            source.bus.read8(WRAM + u32::from(OWNER) + 0x1CC7)
        );
        let expected = before.mode_selection.as_mut().unwrap();
        expected.requested = source.bus.read8(WRAM + SLOT + 0x6AA1);
        expected.transition_control = source.bus.read8(WRAM + SLOT + 0x6BEC);
        assert_eq!(*self.records(), before);
        assert_eq!(
            source.bus.read8(WRAM + SLOT + 0x6AA0),
            before.auxiliary.unwrap().mode
        );
        assert_eq!(source.bus.read8(WRAM + SLOT + 0x6AA2), 0xB2);
        assert_eq!(source.bus.read8(WRAM + u32::from(OWNER) + 0x1CC8), 0x9F);
        for (offset, value) in [
            (0x6BED, return_position.x),
            (0x6BEF, return_position.y),
            (0x6BF1, return_position.z),
        ] {
            assert_eq!(source.bus.read16(WRAM + SLOT + offset) as i16, value);
        }
        actual
    }
}

#[test]
fn pending_mode_selection_matches_original_all_current_modes_control_bytes_and_valid_requests() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    for current in 0..=u8::MAX {
        for pending in 0..5 {
            for control in 0..=u8::MAX {
                let record = f.records();
                record.auxiliary.as_mut().unwrap().mode = current;
                let state = record.mode_selection.as_mut().unwrap();
                state.request_inhibited = true;
                state.requested = (control & 0xF0) | pending;
                state.transition_control = control;
                record.boundary.as_mut().unwrap().return_position = Vector3 {
                    x: u16::from_be_bytes([current, control]) as i16,
                    y: -17,
                    z: 123,
                };
                f.native
                    .objects
                    .get_mut(f.native.owner)
                    .unwrap()
                    .base
                    .behavior_phase = current ^ control;
                f.seed_retained(&mut source);
                f.step(&mut source, ModeRequest::Walker);
            }
        }
    }
}

#[test]
fn new_mode_request_matches_original_all_surface_flags_carry_gates_and_controller_edges() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    for flags in 0..=u8::MAX {
        for gates in 0..16 {
            for request in REQUESTS {
                for carry_mode in [0, 1] {
                    let record = f.records();
                    record.auxiliary.as_mut().unwrap().mode =
                        [0x11, 0x12, 0x24, 0x33][usize::from(flags & 3)];
                    let state = record.mode_selection.as_mut().unwrap();
                    state.surface_control = flags;
                    state.request_inhibited = gates & 1 != 0;
                    state.requested = (flags & 0xF0) | ((flags >> 4) % 5);
                    state.transition_control = if gates & 8 != 0 {
                        0x18 | (flags & !0x18)
                    } else {
                        flags & !0x18
                    };
                    f.native
                        .objects
                        .get_mut(f.native.owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion
                        .carry_selected_player = gates & 2 != 0;
                    f.native.world.player_carry_mode = Some(carry_mode);
                    f.native.world.processed_player_input = Some(InputState {
                        held: Buttons::from_bits(0xEDFF),
                        pressed: Buttons::from_bits(if gates & 4 != 0 {
                            0xFFFF
                        } else {
                            0xFFFF ^ Button::Select as u16
                        }),
                    });
                    f.seed_retained(&mut source);
                    f.step(&mut source, request);
                }
            }
        }
    }
}

#[test]
fn pending_and_new_modes_match_original_continuous_independent_visits() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source);
    f.records().mode_selection.as_mut().unwrap().requested = 0xB4;
    f.seed_retained(&mut source);
    let mut transform_cues = 0;
    let mut same_family = 0;
    for tick in 0_u16..8192 {
        let record = f.records();
        record.auxiliary.as_mut().unwrap().mode =
            [0x11, 0x12, 0x24, 0x33][usize::from((tick / 19) & 3)];
        let state = record.mode_selection.as_mut().unwrap();
        state.surface_control = (tick / 7) as u8;
        state.request_inhibited = tick % 13 == 0;
        f.native.world.player_carry_mode = Some(u8::from(tick % 3 == 0));
        f.native
            .objects
            .get_mut(f.native.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = tick & 1 != 0;
        f.native.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(tick.wrapping_mul(193)),
            pressed: Buttons::from_bits(if tick % 3 == 0 {
                Button::Select as u16
            } else {
                0
            }),
        });
        let prior_requested = f.records().mode_selection.unwrap().requested;
        let result = f.step(&mut source, REQUESTS[usize::from((tick / 11) % 3)]);
        transform_cues += usize::from(result);
        same_family += usize::from(
            prior_requested & 15 != 0 && f.records().mode_selection.unwrap().requested & 15 == 0,
        );
    }
    assert!(transform_cues > 1000 && same_family > 50);
}
