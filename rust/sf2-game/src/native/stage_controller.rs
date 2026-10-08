//! The flight stage's controller, run by the stage loop after each scene
//! frame (`$03:C193` dispatches stage kind 1 to `$03:C500`): the pause
//! trigger (`$03:C70A`), the score publication (`$03:C6E4`) and the scene
//! transition machine (`$03:C509`) on the selector word 1B78, with the
//! running phase's clocks (`$03:C541..C5DE`, `$7F:5F39`) and the shared
//! scene-exit fade (`$03:E0FC`) that ends the stage.
//!
//! The pause state, the planet-destruction branch, the alternate exit
//! (`$03:E154`) and transition phases 3 and 6 are not ported; reaching them
//! faults instead of continuing with an assumed outcome.

use super::cinematic_exit::SceneExitFade;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectStore};

/// 1B84: scripted view (02) or scripted-view transition (10) suspend the
/// running phase; 20 additionally suspends the pause trigger; 01 selects
/// the two-player pause test; 08 redirects the exit to stage kind 0D.
const MODE_SCRIPTED: u16 = 0x0002;
const MODE_TRANSITION: u16 = 0x0010;
const MODE_HELD: u16 = 0x0020;
const MODE_TWO_PLAYER: u16 = 0x0001;
const MODE_ALTERNATE_EXIT: u16 = 0x0008;
/// 1B88 bit 0100: the planet was destroyed. Bit 0002 is cleared by defeat.
const EVENT_PLANET_DESTROYED: u16 = 0x0100;
const EVENT_DEFEAT_CLEARS: u16 = 0x0002;
const EVENT_PLANET_HIT: u16 = 0x1000;
/// 1B86 bit 0004: the stage ended in defeat.
const RESULT_DEFEAT: u16 = 0x0004;
/// D810 bit 80: set by the defeat phase for the HUD service ($04:8584).
const HUD_DEFEAT: u8 = 0x80;
/// 1B9C bit 2000: the planet-hit flash is showing.
const DISPLAY_PLANET_FLASH: u16 = 0x2000;

const BLINK_PERIOD: u16 = 4;
const CLOCK_STEP_FRAMES: u16 = 0xF0;
const CLOCK_LAST_STEP: u16 = 99;
const CLOCK_EXPIRED_FRAMES: u16 = 0xEF;
const ELAPSED_LIMIT: u16 = 999;
const PLANET_FLASH_FRAMES: u16 = 0x32;
const PLANET_FULL_DAMAGE: u16 = 100;

/// Stage kinds selected for the stage loop's next pass (1B6A).
const NEXT_DEFEATED: u16 = 3;
const NEXT_ALTERNATE: u16 = 0x0D;

/// The running phase's clocks ($03:83D0 resets them at stage setup).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageClock {
    /// DA61: frames left in the current step.
    pub step_countdown: u16,
    /// DA63: frames counted in the current step.
    pub step_frames: u16,
    /// DA5B: steps elapsed, saturating at 999.
    pub elapsed_steps: u16,
    /// DA67: frames until the next blink (1B96 bit 0008).
    pub blink_countdown: u16,
}

/// The planet's damage, applied one unit per frame from a pending count
/// (`$7F:5F39`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlanetDamage {
    /// DB4B: damage units still to apply.
    pub pending: u16,
    /// DB47: the planet's remaining health.
    pub health: u16,
    /// DB49: published damage, 100 minus health.
    pub damage: u16,
    /// D9B9: frames left on the planet-hit flash.
    pub flash: u16,
    /// 1B8A bit 0200: damage application is suspended.
    pub suspended: bool,
}

/// The cinematic word's (1B96) bits owned by the stage controller. The
/// word's other bits have their own typed owners in the world.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageSignals {
    /// Bit 0008: on for one frame in every four while the stage runs.
    pub blink: bool,
    /// Bit 8000: the stage clock ran out.
    pub clock_expired: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageControl {
    pub clock: StageClock,
    pub planet: PlanetDamage,
    pub signals: StageSignals,
    /// 1C67 bits 20/10 and 1BBE: the scene-exit fade.
    pub exit_fade: SceneExitFade,
    /// 1C67 bit 04: the alternate exit ($03:E154) is in progress.
    pub alternate_exit: bool,
    /// 1B7C/1B7A: the timed phase's countdown and the phase it continues to.
    pub phase_countdown: u16,
    pub phase_continuation: u16,
    /// 1B6A: the stage kind for the stage loop's next pass.
    pub next_stage: u16,
    /// 1B86: the stage's result flags, read by the campaign owner.
    pub result_flags: u16,
    /// D810: the HUD service's mode byte.
    pub hud_mode: u8,
    /// F532: an action stopped the stage clock ($0D:C695).
    pub clock_stopped: bool,
    /// 1C0E: event bits raised by actor services and cleared on exit.
    pub event_word: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageVisit {
    Continue,
    /// The stage loop leaves for its next stage kind (1B70 = 0).
    Leave { next_stage: u16 },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageOutcome {
    pub visit: Option<StageVisit>,
    /// The exit fade's first visit requests the scene-exit audio.
    pub request_audio_exit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageError {
    World(WorldInputError),
    MissingStageControl,
    MissingSceneDisplay,
    MissingTransition,
    MissingViewMode,
    MissingSceneEvents,
    MissingDisplayFlags,
    MissingObjectiveCounts,
    MissingActionGate,
    MissingEncounterTimer,
    MissingControllerInput,
    MissingPrimaryPlayer,
    MissingScore,
    UnportedPause,
    UnportedPlanetDestruction,
    UnportedAlternateExit,
    UnportedPhase(u16),
}

impl From<WorldInputError> for StageError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// `$03:C500`: one controller visit after the scene frame.
pub fn advance(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<StageOutcome, StageError> {
    apply_map_display(world)?;
    check_pause(world)?;
    publish_score(objects, world)?;
    let stage = world.stage.as_ref().ok_or(StageError::MissingStageControl)?;
    if stage.exit_fade.active {
        return exit_fade(world);
    }
    if stage.alternate_exit {
        return Err(StageError::UnportedAlternateExit);
    }
    let phase = world.scene_transition.ok_or(StageError::MissingTransition)?.phase_word;
    match phase {
        0 => run(world),
        1 => {
            let stage = stage_mut(world)?;
            stage.phase_countdown = stage.phase_countdown.wrapping_sub(1);
            if stage.phase_countdown == 0 {
                let continuation = stage.phase_continuation;
                world.scene_transition.as_mut().ok_or(StageError::MissingTransition)?.phase_word = continuation;
            }
            Ok(StageOutcome::default())
        }
        // $03:C61F: the player was shot down.
        2 => {
            events(world)?.bits &= !EVENT_DEFEAT_CLEARS;
            let stage = stage_mut(world)?;
            stage.result_flags |= RESULT_DEFEAT;
            stage.next_stage = NEXT_DEFEATED;
            stage.hud_mode |= HUD_DEFEAT;
            begin_exit(world)
        }
        // $03:C692.
        4 => {
            const NEXT: u16 = 7;
            const EVENT: u16 = 0x0002;
            stage_mut(world)?.next_stage = NEXT;
            events(world)?.bits |= EVENT;
            begin_exit(world)
        }
        // $03:C63C.
        7 => {
            const NEXT: u16 = 7;
            const EVENT_SET: u16 = 0x4000;
            const EVENT_CLEAR: u16 = 0x0002;
            let events = events(world)?;
            events.bits = (events.bits | EVENT_SET) & !EVENT_CLEAR;
            stage_mut(world)?.next_stage = NEXT;
            begin_exit(world)
        }
        other => Err(StageError::UnportedPhase(other)),
    }
}

/// `$03:C70A`: Start on the playing controller pauses the stage.
fn check_pause(world: &ScenePathWorld) -> Result<(), StageError> {
    let mode = world.view_transition_mode.ok_or(StageError::MissingViewMode)?.flags;
    if mode & (MODE_SCRIPTED | MODE_TRANSITION | MODE_HELD) != 0 {
        return Ok(());
    }
    // The whole objective word, unlike the running phase's low-byte test.
    let objectives = world.objective_counts.ok_or(StageError::MissingObjectiveCounts)?.remaining_word;
    if objectives == 0 {
        return Ok(());
    }
    let pressed = |pad: usize| {
        world.controller_inputs[pad]
            .map(|input| input.pressed.contains(Button::Start))
            .ok_or(StageError::MissingControllerInput)
    };
    let paused = if mode & MODE_TWO_PLAYER != 0 { pressed(0)? || pressed(1)? } else { pressed(0)? };
    if paused {
        return Err(StageError::UnportedPause);
    }
    Ok(())
}

/// `$03:C6E4`: the selected player's score, published for the next stage.
fn publish_score(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<(), StageError> {
    let player = world.primary_player.ok_or(StageError::MissingPrimaryPlayer)?;
    let score = world.player(objects, player)?.score.ok_or(StageError::MissingScore)?;
    world.published_score = Some(score);
    Ok(())
}

/// `$03:C541..C5DE`: the running phase.
fn run(world: &mut ScenePathWorld) -> Result<StageOutcome, StageError> {
    let mode = world.view_transition_mode.ok_or(StageError::MissingViewMode)?.flags;
    let gate = world.action_gate.ok_or(StageError::MissingActionGate)?.code;
    let objectives = world.objective_counts.ok_or(StageError::MissingObjectiveCounts)?.remaining_word as u8;
    let stage = world.stage.as_ref().ok_or(StageError::MissingStageControl)?;
    if mode & (MODE_SCRIPTED | MODE_TRANSITION) != 0 || gate != 0 || objectives == 0 || stage.clock_stopped {
        return Ok(StageOutcome::default());
    }
    if events(world)?.bits & EVENT_PLANET_DESTROYED != 0 {
        return Err(StageError::UnportedPlanetDestruction);
    }
    let stage = stage_mut(world)?;
    stage.signals.blink = false;
    stage.clock.blink_countdown = stage.clock.blink_countdown.wrapping_sub(1);
    if stage.clock.blink_countdown == 0 {
        stage.clock.blink_countdown = BLINK_PERIOD;
        stage.signals.blink = true;
    }
    apply_planet_damage(world)?;
    let stage = stage_mut(world)?;
    if stage.signals.clock_expired {
        return Ok(StageOutcome::default());
    }
    stage.clock.step_frames = stage.clock.step_frames.wrapping_add(1);
    stage.clock.step_countdown = stage.clock.step_countdown.wrapping_sub(1);
    if stage.clock.step_countdown != 0 {
        return Ok(StageOutcome::default());
    }
    let steps = world.encounter_timer_steps.as_mut().ok_or(StageError::MissingEncounterTimer)?;
    let stage = world.stage.as_mut().ok_or(StageError::MissingStageControl)?;
    if *steps == CLOCK_LAST_STEP {
        stage.clock.step_frames = CLOCK_EXPIRED_FRAMES;
        stage.signals.clock_expired = true;
        return Ok(StageOutcome::default());
    }
    stage.clock.step_frames = 0;
    stage.clock.step_countdown = CLOCK_STEP_FRAMES;
    *steps = steps.wrapping_add(1);
    if stage.clock.elapsed_steps != ELAPSED_LIMIT {
        stage.clock.elapsed_steps += 1;
    }
    Ok(StageOutcome::default())
}

/// `$7F:5F39`: one pending unit of planet damage per frame, else the flash.
fn apply_planet_damage(world: &mut ScenePathWorld) -> Result<(), StageError> {
    let planet = world.stage.as_ref().ok_or(StageError::MissingStageControl)?.planet;
    if !planet.suspended && planet.pending != 0 {
        let stage = stage_mut(world)?;
        stage.planet.pending -= 1;
        stage.planet.health = stage.planet.health.wrapping_sub(1);
        stage.planet.flash = PLANET_FLASH_FRAMES;
        stage.planet.damage = PLANET_FULL_DAMAGE.wrapping_sub(stage.planet.health);
        let destroyed = stage.planet.damage == PLANET_FULL_DAMAGE;
        events(world)?.bits |= EVENT_PLANET_HIT;
        *display_flags(world)? |= DISPLAY_PLANET_FLASH;
        if destroyed {
            return Err(StageError::UnportedPlanetDestruction);
        }
        return Ok(());
    }
    if planet.flash != 0 {
        let stage = stage_mut(world)?;
        stage.planet.flash -= 1;
        if stage.planet.flash == 0 {
            *display_flags(world)? &= !DISPLAY_PLANET_FLASH;
        }
    }
    Ok(())
}

/// `$03:C6A0`: start the scene-exit fade, then visit it in the same frame.
fn begin_exit(world: &mut ScenePathWorld) -> Result<StageOutcome, StageError> {
    stage_mut(world)?.exit_fade.active = true;
    exit_fade(world)
}

/// `$03:C6B2..C6DD`: the scene-exit fade, then leave the stage loop.
fn exit_fade(world: &mut ScenePathWorld) -> Result<StageOutcome, StageError> {
    let display = world.scene_display.as_mut().ok_or(StageError::MissingSceneDisplay)?;
    let stage = world.stage.as_mut().ok_or(StageError::MissingStageControl)?;
    let fade = stage.exit_fade.visit(display);
    let mut outcome = StageOutcome {
        visit: None,
        request_audio_exit: fade.request_audio_exit,
    };
    if !fade.completed {
        return Ok(outcome);
    }
    let mode = world.view_transition_mode.ok_or(StageError::MissingViewMode)?.flags;
    let stage = stage_mut(world)?;
    if mode & MODE_ALTERNATE_EXIT != 0 {
        stage.next_stage = NEXT_ALTERNATE;
    }
    stage.event_word = 0;
    let next_stage = stage.next_stage;
    clear_cinematic_word(world);
    world.scene_transition.as_mut().ok_or(StageError::MissingTransition)?.phase_word = 0;
    outcome.visit = Some(StageVisit::Leave { next_stage });
    Ok(outcome)
}

/// `TRB 1B96` with FFFF: every owner of the cinematic word is cleared.
fn clear_cinematic_word(world: &mut ScenePathWorld) {
    if let Some(stage) = world.stage.as_mut() {
        stage.signals = StageSignals::default();
    }
    if let Some(flags) = world.scene_gate_flags.as_mut() {
        *flags = Default::default();
    }
    if let Some(signals) = world.cinematic_signals.as_mut() {
        *signals = Default::default();
    }
    if let Some(inhibited) = world.reticle_inhibited.as_mut() {
        *inhibited = false;
    }
}

fn stage_mut(world: &mut ScenePathWorld) -> Result<&mut StageControl, StageError> {
    world.stage.as_mut().ok_or(StageError::MissingStageControl)
}

fn events(world: &mut ScenePathWorld) -> Result<&mut super::path_scene_state::SceneEventFlags, StageError> {
    world.scene_events.as_mut().ok_or(StageError::MissingSceneEvents)
}

fn display_flags(world: &mut ScenePathWorld) -> Result<&mut u16, StageError> {
    world.scene_display_flags.as_mut().ok_or(StageError::MissingDisplayFlags)
}

/// The display requests the frame's map wrote (F3, F4 and 18BB, stored
/// directly by `$03:9A71/9A7C`, `$03:DD6F` and `$03:993D`). Each is a plain
/// byte store, so the last request of each kind is the stored value.
fn apply_map_display(world: &mut ScenePathWorld) -> Result<(), StageError> {
    use super::map_effects::DisplayModeRequest;
    use super::scene_display::{FadeRequest, Intensity};
    let presentation = &mut world.map_presentation;
    let display = world.scene_display.as_mut().ok_or(StageError::MissingSceneDisplay)?;
    if let Some(mode) = presentation.display_mode.take() {
        display.request = match mode {
            DisplayModeRequest::Blank => FadeRequest::OutFast,
            DisplayModeRequest::Scene => FadeRequest::InFast,
        };
    }
    if std::mem::take(&mut presentation.fade_progress_cleared) {
        display.progress = Intensity::DARK;
    }
    if let Some(hold) = presentation.scene_style.take() {
        display.blank_hold = hold;
    }
    Ok(())
}

/// The fade service that each render's completion interrupt runs
/// (`$7F:0E79`, from `$7F:0846`). Renders complete asynchronously, so a
/// scene frame sees zero, one or more visits; shipping runs one per frame.
pub fn visit_fade(world: &mut ScenePathWorld) -> Result<(), StageError> {
    apply_map_display(world)?;
    let odd = world.strategy_clock & 1 != 0;
    world.scene_display.as_mut().ok_or(StageError::MissingSceneDisplay)?.visit_scene_fade(odd);
    Ok(())
}

/// The stage loop's frame wrapper (`$03:DD81`): the blank hold.
pub fn advance_blank_hold(world: &mut ScenePathWorld) -> Result<(), StageError> {
    apply_map_display(world)?;
    world.scene_display.as_mut().ok_or(StageError::MissingSceneDisplay)?.advance_blank_hold();
    Ok(())
}

/// Before the scene frame's actors: the previous render's fade visit, then
/// the blank hold.
pub fn begin_frame(world: &mut ScenePathWorld) -> Result<(), StageError> {
    visit_fade(world)?;
    advance_blank_hold(world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::path_program::ActionGate;
    use super::super::path_scene_state::{EncounterObjectiveCounts, SceneEventFlags, SceneTransitionControl};
    use super::super::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
    use super::super::view_transition::ViewTransitionMode;

    fn world() -> ScenePathWorld {
        let mut world = ScenePathWorld::new(super::super::RandomState::default());
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0x0080 });
        world.action_gate = Some(ActionGate { code: 0 });
        world.objective_counts = Some(EncounterObjectiveCounts { remaining_word: 1, ..Default::default() });
        world.scene_events = Some(SceneEventFlags::default());
        world.scene_display_flags = Some(0);
        world.encounter_timer_steps = Some(0);
        world.scene_transition = Some(SceneTransitionControl::default());
        world.stage = Some(StageControl {
            clock: StageClock { step_countdown: CLOCK_STEP_FRAMES, step_frames: 0, elapsed_steps: 0, blink_countdown: BLINK_PERIOD },
            ..Default::default()
        });
        world.scene_display = Some(SceneDisplay {
            request: FadeRequest::Idle,
            progress: Intensity::FULL,
            bands: [DisplayBand { blanked: false, intensity: Intensity::FULL }; 3],
            blank_hold: u8::MAX,
            interval_remaining: 0,
            interval_reload: 0,
        });
        world
    }

    #[test]
    fn the_clock_steps_every_240_frames_and_blinks_every_fourth() {
        let mut world = world();
        let mut blinks = 0;
        for _ in 0..CLOCK_STEP_FRAMES {
            run(&mut world).unwrap();
            blinks += u16::from(world.stage.unwrap().signals.blink);
        }
        let stage = world.stage.unwrap();
        assert_eq!(blinks, CLOCK_STEP_FRAMES / BLINK_PERIOD);
        assert_eq!(world.encounter_timer_steps, Some(1));
        assert_eq!(stage.clock, StageClock {
            step_countdown: CLOCK_STEP_FRAMES,
            step_frames: 0,
            elapsed_steps: 1,
            blink_countdown: BLINK_PERIOD,
        });
    }

    #[test]
    fn the_clock_expires_after_its_last_step_and_then_holds() {
        let mut world = world();
        world.encounter_timer_steps = Some(CLOCK_LAST_STEP);
        world.stage.as_mut().unwrap().clock.step_countdown = 1;
        run(&mut world).unwrap();
        let stage = world.stage.unwrap();
        assert!(stage.signals.clock_expired);
        assert_eq!(stage.clock.step_frames, CLOCK_EXPIRED_FRAMES);
        run(&mut world).unwrap();
        // The blink keeps running; the step clock holds.
        let held = world.stage.unwrap().clock;
        assert_eq!((held.step_countdown, held.step_frames, held.elapsed_steps),
            (stage.clock.step_countdown, stage.clock.step_frames, stage.clock.elapsed_steps));
        assert_eq!(world.encounter_timer_steps, Some(CLOCK_LAST_STEP));
    }

    #[test]
    fn the_running_phase_waits_for_the_gate_objectives_and_scripted_view() {
        for setup in [
            |w: &mut ScenePathWorld| w.action_gate = Some(ActionGate { code: 1 }),
            |w: &mut ScenePathWorld| w.objective_counts.as_mut().unwrap().remaining_word = 0x0100,
            |w: &mut ScenePathWorld| w.view_transition_mode.as_mut().unwrap().flags |= MODE_TRANSITION,
            |w: &mut ScenePathWorld| w.stage.as_mut().unwrap().clock_stopped = true,
        ] {
            let mut world = world();
            setup(&mut world);
            let before = world.stage;
            run(&mut world).unwrap();
            assert_eq!(world.stage, before);
        }
    }

    #[test]
    fn pending_planet_damage_lands_one_unit_per_frame_and_flashes() {
        let mut world = world();
        world.stage.as_mut().unwrap().planet = PlanetDamage { pending: 2, health: 50, ..Default::default() };
        run(&mut world).unwrap();
        let planet = world.stage.unwrap().planet;
        assert_eq!((planet.pending, planet.health, planet.damage, planet.flash), (1, 49, 51, PLANET_FLASH_FRAMES));
        assert_eq!(world.scene_display_flags, Some(DISPLAY_PLANET_FLASH));
        assert_eq!(world.scene_events.unwrap().bits, EVENT_PLANET_HIT);
        world.stage.as_mut().unwrap().planet.pending = 0;
        world.stage.as_mut().unwrap().planet.flash = 1;
        run(&mut world).unwrap();
        assert_eq!(world.scene_display_flags, Some(0));
    }

    #[test]
    fn the_exit_fade_leaves_for_the_next_stage_once_the_display_is_dark() {
        let mut world = world();
        world.stage.as_mut().unwrap().next_stage = NEXT_DEFEATED;
        let first = begin_exit(&mut world).unwrap();
        assert!(first.request_audio_exit);
        assert_eq!(first.visit, None);
        assert_eq!(world.scene_display.unwrap().request, FadeRequest::OutFast);
        let mut frames = 0;
        let left = loop {
            visit_fade(&mut world).unwrap();
            frames += 1;
            if let Some(visit) = exit_fade(&mut world).unwrap().visit {
                break visit;
            }
        };
        assert_eq!(frames, 8);
        assert_eq!(left, StageVisit::Leave { next_stage: NEXT_DEFEATED });
        assert!(!world.stage.unwrap().exit_fade.active);
    }
}
