//! The bank-0B presentation director (record 1FBE..1FC9), run once per
//! stage-loop frame before the scene frame (`$0B:8C21`): a requested state
//! (`$0B:AC23`) runs an authored step list (`$0B:AC85`), one step per frame,
//! each step a handler with a one-shot, timed or terminal advance
//! (`$0B:AC51/AC61`, `$0B:ACBC`). Requests come from the stage code through
//! `$0B:8CB0` (holding the stage) and `$0B:8CC1`.
//!
//! Only the stage-start iris (state 3, `$0B:9269`) is ported. Its window
//! handlers program the screen-window HDMA tables; they are published as a
//! typed window command for the renderer. Other states fault.

use super::scene_path_world::ScenePathWorld;

/// 1B84 bit 0020: the stage waits for the director.
const MODE_HELD: u16 = 0x0020;
/// The record's state word: bit 8000 marks a started request.
const STARTED: u16 = 0x8000;
const STAGE_START_IRIS: u16 = 3;
/// Step flags: bit 80 with class 0 (one-shot), 1 (terminal) or 2 (until
/// the handler signals completion through record word 0A).
const FLAGGED: u8 = 0x80;
const CLASS_MASK: u8 = 0x0F;
const TIMER_MASK: u8 = 0x7F;
/// The flight stage's start requests state 1 of the HUD lane ($0B:A836).
const FLIGHT_STAGE_KIND: u16 = 1;
const HUD_LANE_STAGE_START: u16 = 1;

/// The stage-start iris window, as the step handlers program it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum IrisWindow {
    #[default]
    Unchanged,
    /// `$0B:8E64`: the window table filled closed (7F80).
    Closed,
    /// `$0B:9290`: the iris tables installed; the second variant when 1B9C
    /// bit 0008 is set.
    Installed { alternate: bool },
    /// `$0B:92BB`: the iris advanced for this frame (parity from F5CC).
    Opening,
    /// `$0B:8E34`: the window channels disabled.
    Disabled,
    /// `$0B:8E69`: the window table filled open (FF00).
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handler {
    CloseWindow,
    StartStage,
    ShowHud,
    OpenIris,
    ReleaseStage,
    Wait,
    OpenWindow,
    End,
}

/// `$0B:926C`: the stage-start iris steps, `(flags, handler)`.
const STAGE_START_STEPS: [(u8, Handler); 8] = [
    (0x80, Handler::CloseWindow),
    (0x80, Handler::StartStage),
    (0x80, Handler::ShowHud),
    (0x11, Handler::OpenIris),
    (0x80, Handler::ReleaseStage),
    (0x02, Handler::Wait),
    (0x80, Handler::OpenWindow),
    (0x81, Handler::End),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PresentationDirector {
    /// 1FBE: the requested state; bit 8000 once started.
    pub request: u16,
    /// 1FC0: the running state.
    pub state: u16,
    /// 1FC2/1FC4/1FC6: the step, its timer and frames elapsed in it.
    pub step: u16,
    pub timer: u16,
    pub elapsed: u16,
    /// 1FC8: a handler's completion signal for class-2 steps.
    pub completion: u16,
    /// F5CC: frames the director has run.
    pub frame_counter: u16,
    /// F53E: the HUD is shown.
    pub hud_shown: bool,
    /// 1FD2: the state word of the director's HUD lane (record 1FD2..1FDB),
    /// requested at the flight stage's start. The lane itself runs from the
    /// HUD service (`$04:93AC` -> `$0B:A842`), which is not ported.
    pub hud_lane_request: u16,
    /// The last window command this frame.
    pub window: IrisWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorError {
    MissingDirector,
    MissingViewMode,
    MissingStageControl,
    MissingDisplayFlags,
    UnportedState(u16),
}

/// `$0B:8CB0`/`$0B:8CC1`: request a state. A holding request also hides
/// the HUD and holds the stage until the state releases it.
pub fn request(world: &mut ScenePathWorld, state: u8, hold: bool) -> Result<(), DirectorError> {
    if hold {
        world.view_transition_mode.as_mut().ok_or(DirectorError::MissingViewMode)?.flags |= MODE_HELD;
    }
    let director = world.director.as_mut().ok_or(DirectorError::MissingDirector)?;
    director.hud_shown = !hold;
    director.request = u16::from(state);
    Ok(())
}

/// `$0B:8C21`: one director frame.
pub fn advance(world: &mut ScenePathWorld, stage_kind: u16) -> Result<(), DirectorError> {
    let director = world.director.as_mut().ok_or(DirectorError::MissingDirector)?;
    director.window = IrisWindow::Unchanged;
    if director.request != 0 {
        if director.request & STARTED == 0 {
            director.state = director.request;
            director.request |= STARTED;
            director.step = 0;
            director.elapsed = 0;
            director.timer = 0;
        }
        if director.state != STAGE_START_IRIS {
            return Err(DirectorError::UnportedState(director.state));
        }
        run_step(world, &STAGE_START_STEPS, stage_kind)?;
    }
    let director = world.director.as_mut().ok_or(DirectorError::MissingDirector)?;
    director.frame_counter = director.frame_counter.wrapping_add(1);
    Ok(())
}

/// `$0B:AC85`: run the current step's handler, then advance it.
fn run_step(world: &mut ScenePathWorld, steps: &[(u8, Handler)], stage_kind: u16) -> Result<(), DirectorError> {
    let director = world.director.as_mut().ok_or(DirectorError::MissingDirector)?;
    director.completion = 0;
    let step = usize::from(director.step);
    let Some(&(flags, handler)) = steps.get(step) else {
        // A zero flag byte ends the list ($0B:ACC2 -> $0B:AC78).
        director.request = 0;
        director.state = 0;
        director.step = 0;
        return Ok(());
    };
    let next = steps.get(step + 1).map_or(0, |&(flags, _)| flags & TIMER_MASK);
    run_handler(world, handler, stage_kind)?;
    let director = world.director.as_mut().ok_or(DirectorError::MissingDirector)?;
    if flags & FLAGGED == 0 {
        if director.timer != 0 {
            director.timer -= 1;
            director.elapsed = director.elapsed.saturating_add(1);
        } else {
            next_step(director, next);
        }
        return Ok(());
    }
    match flags & CLASS_MASK {
        0 => next_step(director, next),
        1 => director.elapsed = director.elapsed.saturating_add(1),
        _ => {
            director.elapsed = director.elapsed.saturating_add(1);
            if director.completion != 0 {
                next_step(director, next);
            }
        }
    }
    Ok(())
}

/// `$0B:AC61`: the next step starts with the following entry's timer.
fn next_step(director: &mut PresentationDirector, timer: u8) {
    director.timer = u16::from(timer);
    director.elapsed = 0;
    director.step += 1;
}

fn run_handler(world: &mut ScenePathWorld, handler: Handler, stage_kind: u16) -> Result<(), DirectorError> {
    match handler {
        Handler::CloseWindow => director(world)?.window = IrisWindow::Closed,
        // $0B:A81E: the stage clock runs; the flight stage requests its HUD
        // lane's first state and empties the bank-0B sprite list ($0B:C850). That
        // sprite system draws only and is not modeled, so nothing native
        // holds sprites to release.
        Handler::StartStage => {
            world.stage.as_mut().ok_or(DirectorError::MissingStageControl)?.clock_stopped = false;
            if stage_kind == FLIGHT_STAGE_KIND {
                director(world)?.hud_lane_request = HUD_LANE_STAGE_START;
            }
            install_iris(world)?;
        }
        // $0B:92B2.
        Handler::ShowHud => {
            director(world)?.hud_shown = true;
            install_iris(world)?;
        }
        Handler::OpenIris => director(world)?.window = IrisWindow::Opening,
        Handler::ReleaseStage => {
            director(world)?.window = IrisWindow::Disabled;
            world.view_transition_mode.as_mut().ok_or(DirectorError::MissingViewMode)?.flags &= !MODE_HELD;
        }
        Handler::Wait => {}
        Handler::OpenWindow => director(world)?.window = IrisWindow::Open,
        // $0B:8E2C: the record's state, running state and step are cleared.
        Handler::End => {
            let director = director(world)?;
            director.request = 0;
            director.state = 0;
            director.step = 0;
        }
    }
    Ok(())
}

/// `$0B:9290`.
fn install_iris(world: &mut ScenePathWorld) -> Result<(), DirectorError> {
    const DISPLAY_ALTERNATE_IRIS: u16 = 0x0008;
    let flags = world.scene_display_flags.ok_or(DirectorError::MissingDisplayFlags)?;
    director(world)?.window = IrisWindow::Installed { alternate: flags & DISPLAY_ALTERNATE_IRIS != 0 };
    Ok(())
}

fn director(world: &mut ScenePathWorld) -> Result<&mut PresentationDirector, DirectorError> {
    world.director.as_mut().ok_or(DirectorError::MissingDirector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::stage_controller::StageControl;
    use super::super::view_transition::ViewTransitionMode;

    #[test]
    fn the_stage_start_iris_holds_the_stage_for_twenty_one_frames() {
        let mut world = ScenePathWorld::new(super::super::RandomState::default());
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0x0080 });
        world.stage = Some(StageControl { clock_stopped: true, ..Default::default() });
        world.scene_display_flags = Some(0x000E);
        world.director = Some(PresentationDirector::default());
        request(&mut world, 3, true).unwrap();
        assert_eq!(world.view_transition_mode.unwrap().flags, 0x00A0);
        assert!(!world.director.unwrap().hud_shown);
        // (step, timer, elapsed) after each frame, as retail runs it.
        let mut timeline = vec![(1, 0, 0), (2, 0, 0), (3, 0x11, 0)];
        timeline.extend((1..=0x11).map(|elapsed| (3, 0x11 - elapsed, elapsed)));
        timeline.extend([(4, 0, 0), (5, 2, 0), (5, 1, 1), (5, 0, 2), (6, 0, 0), (7, 1, 0)]);
        for (frame, &expected) in timeline.iter().enumerate() {
            advance(&mut world, FLIGHT_STAGE_KIND).unwrap();
            let director = world.director.unwrap();
            assert_eq!((director.step, director.timer, director.elapsed), expected, "frame {}", frame + 1);
            assert_eq!(director.state, 3);
            let held = world.view_transition_mode.unwrap().flags & MODE_HELD != 0;
            assert_eq!(held, frame + 1 < 22, "frame {}", frame + 1);
            assert_eq!(director.hud_shown, frame + 1 >= 3);
        }
        assert!(!world.stage.unwrap().clock_stopped);
        advance(&mut world, FLIGHT_STAGE_KIND).unwrap();
        let director = world.director.unwrap();
        assert_eq!((director.request, director.state, director.step, director.timer, director.elapsed), (0, 0, 0, 1, 1));
        assert_eq!(director.hud_lane_request, HUD_LANE_STAGE_START);
        let idle = director;
        advance(&mut world, FLIGHT_STAGE_KIND).unwrap();
        assert_eq!(world.director.unwrap().frame_counter, idle.frame_counter + 1);
        assert_eq!(world.director.unwrap().step, idle.step);
    }
}
