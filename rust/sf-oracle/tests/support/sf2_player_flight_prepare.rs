//! Complete caller prefix, stopping at its actual movement-call boundary.
//! Consecutive visits retain each implementation's own camera/input state.
use super::*;
use sf2_game::hit_response::HitSide;
use sf2_game::player_flight_prepare;
use sf2_game::player_input::PlayerInputSettings;
use sf2_game::player_roll::{PlayerRoll, ShoulderControl};
use sf2_game::{Buttons, InputState};

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

impl DispatchFixture {
    fn prepare_varied(&mut self, word: u16, style: Option<Style>, task: Task) {
        self.varied(word, style);
        let f = &mut self.auxiliary.ground.position.height.camera.inner;
        f.records().camera_auxiliary.as_mut().unwrap().task = task;
        f.records().roll = Some(PlayerRoll {
            shoulders: ShoulderControl::from_bits(word as u8),
            tap_window: word.rotate_left(3) as u8,
            impulse: (word >> 8) as i8,
        });
        f.records().injected_input = Some(input(word.wrapping_mul(37), word ^ 0x956A));
        f.records().contact.as_mut().unwrap().ignores_contacts = word & 1 == 0;
        f.world.controller_inputs = [
            Some(input(word, !word)),
            Some(input(word.rotate_left(5), word.rotate_right(3))),
        ];
        f.world.player_input_settings = Some(PlayerInputSettings {
            flight_style: word as u8,
            button_layout: (word >> 8) as u8,
        });
        f.world.unmasked_player_input = Some(input(word ^ 0xA5A5, word ^ 0x5A5A));
        let actor = f.objects.get_mut(f.owner).unwrap();
        actor.base.contacts.hit_side = if word & 256 == 0 {
            HitSide::Primary
        } else {
            HitSide::Secondary
        };
        actor.extension.path_state.conditions.hit_event_pending = word & 2 == 0;
    }

    fn prepare_values(&self) -> Vec<(u32, u16, bool)> {
        let f = &self.auxiliary.ground.position.height.camera.inner;
        let r = f.world.player(&f.objects, f.owner).unwrap();
        let actor = f.objects.get(f.owner).unwrap();
        let roll = r.roll.unwrap();
        let settings = f.world.player_input_settings.unwrap();
        let mut values = vec![
            (
                WRAM + u32::from(OWNER) + 0x23,
                u16::from(
                    0xB7 | u8::from(actor.extension.path_state.conditions.hit_event_pending) * 8
                        | u8::from(actor.base.contacts.hit_side == HitSide::Secondary) * 0x40,
                ),
                true,
            ),
            (WRAM + SLOT + 0x6B7E, u16::from(roll.shoulders.bits()), true),
            (WRAM + SLOT + 0x6ADC, u16::from(roll.tap_window), true),
            (WRAM + SLOT + 0x6ADD, u16::from(roll.impulse as u8), true),
            (WRAM + 0x1DCF, u16::from(settings.flight_style), true),
            (WRAM + 0x1DD0, u16::from(settings.button_layout), true),
        ];
        for (held, pressed, value) in [
            (0x1292, 0x1296, f.world.controller_inputs[0].unwrap()),
            (0x1294, 0x1298, f.world.controller_inputs[1].unwrap()),
            (SLOT + 0x6A8A, SLOT + 0x6A88, r.injected_input.unwrap()),
            (0x1938, 0x1936, f.world.processed_player_input.unwrap()),
            (0x1DA7, 0x1DA9, f.world.unmasked_player_input.unwrap()),
        ] {
            values.push((WRAM + held, value.held.bits(), false));
            values.push((WRAM + pressed, value.pressed.bits(), false));
        }
        values
    }

    fn prepare_seed(&mut self, source: &mut Source) {
        self.seed(source);
        for (field, value, byte) in self.prepare_values() {
            if byte {
                source.bus.write8(field, value as u8);
            } else {
                source.bus.write16(field, value);
            }
        }
    }

    fn prepare_step(&mut self, source: &mut Source) {
        source.run(0x06869C, Some(0x06875F), 0, OWNER, true);
        let f = &mut self.auxiliary.ground.position.height.camera.inner;
        player_flight_prepare::prepare(
            &mut f.objects,
            &mut f.world,
            &mut self.auxiliary.ground.runtime,
            f.owner,
        )
        .unwrap();
        self.verify(source, "free-flight preparation");
        for (field, value, byte) in self.prepare_values() {
            let original = if byte {
                u16::from(source.bus.read8(field))
            } else {
                source.bus.read16(field)
            };
            assert_eq!(value, original, "free-flight preparation field={field:06X}");
        }
    }
}

#[test]
fn original_flight_prepare_all_controller_words_preserve_installed_camera_and_consume_injection_once(
) {
    let mut source = Source::new(&rom(), 0xA7);
    let mut f = DispatchFixture::new(&mut source);
    for word in 0..=u16::MAX {
        f.prepare_varied(word, Some(Style::Normal), Task::None);
        let inner = &mut f.auxiliary.ground.position.height.camera.inner;
        inner.records().auxiliary.as_mut().unwrap().action_flags |= 1;
        inner
            .records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = false;
        f.prepare_seed(&mut source);
        f.prepare_step(&mut source);
        f.prepare_step(&mut source);
    }
}

#[test]
fn original_flight_prepare_all_camera_tasks_modes_and_control_bytes_match_repeated_visits() {
    let mut source = Source::new(&rom(), 0x59);
    let mut f = DispatchFixture::new(&mut source);
    for style in [
        None,
        Some(Style::Normal),
        Some(Style::ProjectionCorrected),
        Some(Style::Surface),
    ] {
        for task in TASKS {
            for byte in 0..=u8::MAX {
                let word = u16::from(byte) | u16::from(byte.reverse_bits()) << 8;
                f.prepare_varied(word, style, task);
                f.auxiliary
                    .ground
                    .position
                    .height
                    .camera
                    .inner
                    .world
                    .view_transition_mode
                    .as_mut()
                    .unwrap()
                    .set_active(byte & 32 != 0);
                f.prepare_seed(&mut source);
                f.prepare_step(&mut source);
                f.prepare_step(&mut source);
            }
        }
    }
}
