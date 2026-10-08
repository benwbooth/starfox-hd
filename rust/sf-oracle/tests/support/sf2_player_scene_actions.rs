//! Original indexed action streams, including the non-null empty program.
//! The dispatcher and every service execute unchanged; only common input
//! state is written before a case, never an output from the other engine.
use super::{action_tests::Fixture, rom, Source, WRAM};
use sf2_game::cinematic_exit::CinematicSignals;
use sf2_game::player_action::{AuthoredSceneAction, PlayerAction};

const SLOT: u32 = 64;
const ACTIONS: [AuthoredSceneAction; 6] = [
    AuthoredSceneAction::Scene3,
    AuthoredSceneAction::Scene4,
    AuthoredSceneAction::Scene5,
    AuthoredSceneAction::Scene7,
    AuthoredSceneAction::Scene9,
    AuthoredSceneAction::Scene25,
];

#[test]
fn original_scene_actions_match_every_elapsed_word_pause_state_and_retained_flag() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for action in ACTIONS {
        f.prepare(Some(PlayerAction::Scene(action)), 0, 0);
        f.seed(&mut source, false);
        for elapsed in 0..=u16::MAX {
            for paused in [false, true] {
                // These are new common inputs to the case. All other source
                // and typed fields remain independent retained state.
                let count = elapsed.rotate_left(9);
                let age = !elapsed;
                let record = f.records().action.as_mut().unwrap();
                record.elapsed = elapsed;
                record.auxiliary_counter = count;
                record.total_updates = age;
                let flags = 0xA55D | if paused { 2 } else { 0 };
                f.world.view_transition_mode.as_mut().unwrap().flags = flags;
                let signals = CinematicSignals {
                    exit_requested: elapsed & 4 != 0,
                    skip_ready: elapsed & 8 != 0,
                };
                f.world.cinematic_signals = Some(signals);
                // Phase bytes cover zero, the compare operand, the sign boundary
                // of the wrapped difference and its neighbours.
                let phase =
                    [0u8, 1, 2, 0x80, 0x81, 0xFF, 0x7F, 0x82][usize::from(elapsed >> 5) & 7];
                f.world.campaign_phase = Some(phase);
                source.bus.write8(WRAM + 0x1BE0, phase);
                f.world.scene_progress_flag = Some(elapsed as u8 ^ 0x3C);
                source.bus.write8(WRAM + 0x1E66, elapsed as u8 ^ 0x3C);
                f.world.audio.take_music_control();
                source.bus.write8(WRAM + 0x1CDA, 201);
                source.bus.write8(WRAM + 0x1CD9, 231);
                let corrected = elapsed & 16 != 0;
                f.records()
                    .camera_dispatch
                    .as_mut()
                    .unwrap()
                    .projection_correction_disabled = corrected;
                for (address, value) in [
                    (SLOT + 0x6C16, elapsed),
                    (SLOT + 0x6C18, count),
                    (SLOT + 0x6C1A, age),
                    (0x1B84, flags),
                    (
                        0x1B96,
                        0xABCF
                            | u16::from(signals.exit_requested) * 0x10
                            | u16::from(signals.skip_ready) * 0x20,
                    ),
                ] {
                    source.bus.write16(WRAM + address, value);
                }
                source
                    .bus
                    .write8(WRAM + SLOT + 0x6B65, 0xBF | u8::from(corrected) * 0x40);
                f.visit(&mut source, false);
            }
        }
    }
}

#[test]
fn original_scene_actions_keep_identity_and_clocks_across_exit_request_and_external_acknowledgement(
) {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for action in ACTIONS {
        f.prepare(Some(PlayerAction::Scene(action)), 0, 397);
        f.world.view_transition_mode.as_mut().unwrap().flags = 0;
        f.world.cinematic_signals = Some(CinematicSignals {
            exit_requested: false,
            skip_ready: true,
        });
        f.records()
            .camera_dispatch
            .as_mut()
            .unwrap()
            .projection_correction_disabled = false;
        f.seed(&mut source, false);
        for visit in 0..260 {
            if visit == 1 {
                // Another camera service may re-enable correction. The
                // action must not repeat its time-zero publication later.
                source.bus.write8(WRAM + SLOT + 0x6B65, 0xBF);
                f.records()
                    .camera_dispatch
                    .as_mut()
                    .unwrap()
                    .projection_correction_disabled = false;
            }
            if [125, 145, 181, 228].contains(&visit) {
                // Acknowledge the shared exit bit without changing the action
                // clock. Later visits cannot turn an At command into From.
                source.bus.write16(WRAM + 0x1B96, 0xABEF);
                f.world.cinematic_signals.as_mut().unwrap().exit_requested = false;
            }
            f.visit(&mut source, false);
        }
    }
}
