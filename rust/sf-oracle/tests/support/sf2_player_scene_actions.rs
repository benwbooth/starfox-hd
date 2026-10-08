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

/// Scene six (`$0D:BEDF`): palette snapshot/highlight, flash and restore
/// intervals (including the double restore), five gate steps that restart
/// total updates, and the exit request. Every update runs on both engines.
#[test]
fn original_scene_six_stream_matches_palette_gate_and_exit_on_every_update() {
    use sf2_game::path_program::ActionGate;
    use sf2_game::player_action::ScenePalette;
    use sf2_game::player_consumable::{PlayerConsumableControl, TriggeredUseBlockers};
    use sf2_game::player_palette::PlayerPaletteControl;

    let mut source = Source::new(&rom(), 0);
    for (case, (flags, gate)) in [(0x00u8, 1u8), (0x58, 0xFE), (0xF8, 0x40), (0x67, 0xFF)]
        .into_iter()
        .enumerate()
    {
        let mut f = Fixture::new();
        f.prepare(Some(PlayerAction::Scene(AuthoredSceneAction::Scene6)), 0, 0);
        f.world.view_transition_mode.as_mut().unwrap().flags = 0;
        f.world.cinematic_signals = Some(CinematicSignals::default());
        f.records().palette_effects = Some(PlayerPaletteControl::from_control(flags));
        f.records().consumable = Some(PlayerConsumableControl {
            projectile_blockers: TriggeredUseBlockers::from_control(flags),
            ..Default::default()
        });
        let untouched = flags & 7;
        f.world.action_gate = Some(ActionGate { code: gate });
        f.world.palette_refresh_requested = Some(false);
        f.world.primary_player = Some(f.owner);
        let seed = case as u16 * 977;
        f.world.palette = Some(ScenePalette {
            colors: std::array::from_fn(|i| (i as u16).wrapping_mul(2963) ^ seed),
            saved_colors: std::array::from_fn(|i| (i as u16).wrapping_mul(613) ^ !seed),
        });
        f.seed(&mut source, false);
        source.bus.write8(WRAM + SLOT + 0x6BE9, flags);
        source.bus.write8(WRAM + 0x1D72, gate);
        source.bus.write8(WRAM + 0x1E58, 0x37);
        let palette = f.world.palette.clone().unwrap();
        for (index, (&color, &saved)) in palette.colors.iter().zip(&palette.saved_colors).enumerate() {
            source.bus.write16(WRAM + 0xEFE5 + index as u32 * 2, color);
            source.bus.write16(WRAM + 0xF2E5 + index as u32 * 2, saved);
        }
        for visit in 0..470u16 {
            f.visit(&mut source, false);
            let r = f.records().clone();
            let context = format!("case {case} visit {visit}");
            assert_eq!(
                source.bus.read8(WRAM + SLOT + 0x6BE9),
                r.palette_effects.unwrap().bits()
                    | r.consumable.unwrap().projectile_blockers.bits()
                    | untouched,
                "{context}"
            );
            assert_eq!(source.bus.read8(WRAM + 0x1D72), f.world.action_gate.unwrap().code, "{context}");
            assert_eq!(
                source.bus.read8(WRAM + 0x1E58),
                0x37 | u8::from(f.world.palette_refresh_requested.unwrap()) * 0x80,
                "{context}"
            );
            assert_eq!(
                source.bus.read8(WRAM + 0x1B96) & 0x10 != 0,
                f.world.cinematic_signals.unwrap().exit_requested,
                "{context}"
            );
            let palette = f.world.palette.as_ref().unwrap();
            for (index, (&color, &saved)) in palette.colors.iter().zip(&palette.saved_colors).enumerate() {
                let offset = index as u32 * 2;
                assert_eq!(source.bus.read16(WRAM + 0xEFE5 + offset), color, "{context} color {index}");
                assert_eq!(source.bus.read16(WRAM + 0xF2E5 + offset), saved, "{context} saved {index}");
            }
        }
    }
}
