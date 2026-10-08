//! Player main strategy (`$06:9C27`): the ordered visit prefix, the phase
//! dispatch on the actor's behavior phase (1CC7, table `$06:9D1A`), and the
//! retained-pitch flight phases 7 (`$06:87A9`) and 8 (`$06:8807..87A8`) with
//! the common flight tail (`$06:877F..87A8`, `$06:9DBA..A31C`).
//!
//! Every step calls the existing source-bounded service at its source call
//! site. Phases and branches that are not ported yet fault visibly with the
//! source address of the first unported instruction; nothing is skipped.

use super::path_relationships;
use super::player_camera_tracking::TrackingStyle;
use super::player_flight_mode::FlightModeContext;
use super::player_mission::MissionAdmission;
use super::scene_path_world::WorldInputError;
use super::scene_strategy::{SceneActors, SceneCallbacks, SceneError};
use super::{Button, ObjectId, ShapeId};

/// Retained-pitch flight phase entry and its continuing phase.
const ENTER_RETAINED_PITCH: u8 = 7;
const RETAINED_PITCH: u8 = 8;
/// `$06:87B3`: retained-pitch flight movement class.
const RETAINED_PITCH_MODE: u8 = 0x12;
/// `$06:87BC`: shared view control bit 20 (1DE0).
const VIEW_CONTROL_FLIGHT: u8 = 0x20;
/// `$06:87CF`: collision-mode search bits of 1B4D.
const SURFACE_SEARCH_BITS: u8 = 0x07;
/// `$06:883C`: auxiliary action bit 04 selects the uncorrected camera.
const ACTION_UNCORRECTED_CAMERA: u8 = 0x04;
/// `$06:8855`: surface/carry bit 08 of 6B64.
const SURFACE_CAMERA_PENDING: u8 = 0x08;
/// `$06:8784`: flight-form bit 80 of 6A72.
const FORM_FLIGHT: u8 = 0x80;
/// `$06:9692`: pilot craft shapes, indexed by the pilot pair (`$06:90D4`
/// limits the pilot byte to the six-entry roster, then `$06:90F8` drops bit
/// 0). ShapeHdr tokens C24C, C5E8, C94C.
const PILOT_PAIR_SHAPES: [ShapeId; 3] = [
    ShapeId::from_catalog_index(52),
    ShapeId::from_catalog_index(85),
    ShapeId::from_catalog_index(116),
];
const PILOT_ROSTER: u8 = 6;
/// `$07:D78B`/`$07:D79A`: numbered children that block consumable use.
const CONSUMABLE_BLOCKING_CHILDREN: [u8; 2] = [0x12, 0x16];
const CONSUMABLE_DELAY_MASK: u8 = 0x0F;
const CONSUMABLE_REUSE_DELAY: u8 = 0x05;
/// `$06:A0E3`: transition-control bit 10 of 6BEC.
const EXIT_TRANSITION: u8 = 0x10;
/// `$06:A0C7`: execution-mode bit 0002 (1B84).
const EXIT_SUPPRESSED_MODE: u16 = 0x0002;
/// Handoff (1D74) bits read by the stage-exit gate (`$06:A129`).
const HANDOFF_EXIT_ACTIVE: u8 = 0x10;
const HANDOFF_EXIT_REQUESTED: u8 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStrategyError {
    World(WorldInputError),
    /// The dispatch selected a phase whose strategy is not ported.
    UnportedPhase(u8),
    /// `$07:D8A3`: the consumable placement and use helper.
    UnportedConsumableUse,
    /// `$0D:BBB6`: the exit-controller installer.
    UnportedExitController,
    /// `$06:A13D`: the stage-exit continuation.
    UnportedStageExit,
    MissingProcessedInput,
    MissingAuxiliary(ObjectId),
    MissingModeSelection(ObjectId),
    MissingCameraDispatch(ObjectId),
    MissingViewControl,
    MissingSurfaceMode,
    MissingFixedView,
    MissingConsumableControl(ObjectId),
    MissingAction(ObjectId),
    MissingActionGate,
    MissingPlayerServices,
    MissingViewMode,
    MissingObjectiveState,
    MissingHandoff,
    MissingRecord(&'static str),
    Relationships(path_relationships::RelationshipError),
}

impl From<WorldInputError> for PlayerStrategyError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

impl<C: SceneCallbacks> SceneActors<'_, C> {
    fn strategy_error(&mut self, error: PlayerStrategyError) -> SceneError<C::Error> {
        self.execution.faulted = true;
        SceneError::PlayerStrategy(error)
    }

    /// `$06:9C27`: one admitted visit of the player main strategy.
    pub fn advance_player_strategy(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.is_faulted() {
            return Err(SceneError::Faulted);
        }
        // The prefix's action stream observes the previous visit's processed
        // input; this visit's input is published later by the phase.
        let input = match self.world.processed_player_input {
            Some(input) => input,
            None => return Err(self.strategy_error(PlayerStrategyError::MissingProcessedInput)),
        };
        self.begin_player_visit(owner, input)?;
        let phase = self
            .objects
            .get(owner)
            .ok_or(SceneError::MissingActor(owner))?
            .base
            .behavior_phase;
        match phase {
            ENTER_RETAINED_PITCH => {
                self.enter_retained_pitch_flight(owner)
                    .map_err(|error| self.strategy_error(error))?;
                self.advance_retained_pitch_flight(owner)
            }
            RETAINED_PITCH => self.advance_retained_pitch_flight(owner),
            _ => Err(self.strategy_error(PlayerStrategyError::UnportedPhase(phase))),
        }
    }

    /// `$06:87A9..8806`: install retained-pitch flight, then fall into it.
    fn enter_retained_pitch_flight(&mut self, owner: ObjectId) -> Result<(), PlayerStrategyError> {
        let world = &mut *self.world;
        let objects = &mut *self.objects;
        world.scene.player_walker_form = Some(false);
        world
            .player_mut(objects, owner)?
            .auxiliary
            .as_mut()
            .ok_or(PlayerStrategyError::MissingAuxiliary(owner))?
            .mode = RETAINED_PITCH_MODE;
        *world
            .scene
            .player_view_control
            .as_mut()
            .ok_or(PlayerStrategyError::MissingViewControl)? |= VIEW_CONTROL_FLIGHT;
        world.player_view_options_enabled = Some(true);
        reset_flight_entry(objects, world, owner)?;
        world
            .surface_mode
            .as_mut()
            .ok_or(PlayerStrategyError::MissingSurfaceMode)?
            .flags &= !SURFACE_SEARCH_BITS;
        objects
            .get_mut(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .behavior_phase = RETAINED_PITCH;
        // Fixed-view base 21 bits 08 and 10.
        let view = world.fixed_players[0].ok_or(PlayerStrategyError::MissingFixedView)?;
        let motion = &mut objects
            .get_mut(view)
            .ok_or(WorldInputError::MissingActor(view))?
            .extension
            .path_state
            .motion;
        motion.follow_player_displacement = true;
        motion.generate_velocity_each_step = true;
        // `$06:883C`: profile, primary camera task and surface camera bit.
        super::player_view_distance::initialize(objects, world, owner)
            .map_err(|_| PlayerStrategyError::MissingRecord("view distance"))?;
        let uncorrected = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(PlayerStrategyError::MissingAuxiliary(owner))?
            .action_flags
            & ACTION_UNCORRECTED_CAMERA
            != 0;
        let records = world.player_mut(objects, owner)?;
        records
            .mode_selection
            .as_mut()
            .ok_or(PlayerStrategyError::MissingModeSelection(owner))?
            .surface_control &= !SURFACE_CAMERA_PENDING;
        records
            .camera_dispatch
            .as_mut()
            .ok_or(PlayerStrategyError::MissingCameraDispatch(owner))?
            .style = Some(if uncorrected {
            TrackingStyle::Normal
        } else {
            TrackingStyle::ProjectionCorrected
        });
        // Base 20 bit 08 has no recovered reader; base 24 bit 02 is the
        // surface relationship admission.
        objects
            .get_mut(owner)
            .expect("validated player")
            .base
            .flags
            .standing_on_surface = false;
        Ok(())
    }

    /// `$06:8807..8839`, then the common tail `$06:877F`.
    fn advance_retained_pitch_flight(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        self.world
            .player_mut(self.objects, owner)
            .map_err(SceneError::World)?
            .contact
            .as_mut()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerContact(
                owner,
            )))?
            .ignores_contacts = false;
        self.objects
            .get_mut(owner)
            .ok_or(SceneError::MissingActor(owner))?
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        self.prepare_player_input(owner)?;
        self.prepare_player_shoulders(owner)?;
        let context = FlightModeContext {
            flight: super::player_flight::FlightContext {
                occupancy: super::player_occupancy::OccupancyContext {
                    camera_override_active: Some(self.camera_task_installed(owner)?),
                    diagonal_tie_bias: None,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        self.advance_player_retained_pitch_flight(owner, context)?;
        self.advance_player_view(owner)?;
        let pilot = self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .visit
            .ok_or(SceneError::World(WorldInputError::MissingAuxiliary(owner)))?
            .pilot_code;
        let pilot = if pilot < PILOT_ROSTER { pilot } else { 0 };
        self.objects
            .get_mut(owner)
            .ok_or(SceneError::MissingActor(owner))?
            .base
            .shape = PILOT_PAIR_SHAPES[usize::from(pilot >> 1)];
        self.world
            .player_mut(self.objects, owner)
            .map_err(SceneError::World)?
            .mode_selection
            .as_mut()
            .ok_or(SceneError::PlayerStrategy(
                PlayerStrategyError::MissingModeSelection(owner),
            ))?
            .form_control |= FORM_FLIGHT;
        self.advance_flight_tail(owner)
    }

    fn camera_task_installed(&mut self, owner: ObjectId) -> Result<bool, SceneError<C::Error>> {
        Ok(self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .camera_auxiliary
            .ok_or(SceneError::World(WorldInputError::MissingAuxiliary(owner)))?
            .task
            != super::player_camera_auxiliary::AuxiliaryCameraTask::None)
    }

    /// `$06:8789..87A8`, then `JML $06:9DBA` through the strategy's RTL.
    fn advance_flight_tail(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        self.advance_player_shield_status(owner)?;
        self.advance_player_status_filters(owner)?;
        self.advance_player_engine_sound(owner)?;
        self.advance_player_transform_cues(owner)?;
        self.prepare_player_reticle(owner)?;
        self.advance_player_weapons(owner)?;
        let input = self.processed_input()?;
        self.advance_player_charge(owner, input)?;
        self.advance_player_strategy_tail(owner)
    }

    fn processed_input(&mut self) -> Result<super::InputState, SceneError<C::Error>> {
        match self.world.processed_player_input {
            Some(input) => Ok(input),
            None => Err(self.strategy_error(PlayerStrategyError::MissingProcessedInput)),
        }
    }

    /// `$07:D6CC..D8A2`: aim publication, the consumable gate and rapid fire.
    fn advance_player_weapons(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        self.publish_player_weapon_aim(owner)?;
        self.gate_player_consumable(owner)
            .map_err(|error| self.strategy_error(error))?;
        let input = self.processed_input()?;
        self.advance_player_rapid(owner, input)
    }

    /// `$07:D78B..D7E3`. Children 12/16 and an installed action block use;
    /// a running delay counts down; otherwise X starts the placement helper.
    fn gate_player_consumable(&mut self, owner: ObjectId) -> Result<(), PlayerStrategyError> {
        for number in CONSUMABLE_BLOCKING_CHILDREN {
            if path_relationships::find_direct_child(self.objects, owner, number)
                .map_err(PlayerStrategyError::Relationships)?
                .is_some()
            {
                return Ok(());
            }
        }
        let records = self.world.player_mut(self.objects, owner)?;
        if records
            .action
            .ok_or(PlayerStrategyError::MissingAction(owner))?
            .action
            .is_some()
        {
            return Ok(());
        }
        let control = &mut records
            .consumable
            .as_mut()
            .ok_or(PlayerStrategyError::MissingConsumableControl(owner))?
            .input_control;
        if *control & CONSUMABLE_DELAY_MASK != 0 {
            *control = control.wrapping_sub(1);
            return Ok(());
        }
        let pressed = self
            .world
            .processed_player_input
            .ok_or(PlayerStrategyError::MissingProcessedInput)?
            .pressed;
        if !pressed.contains(Button::X) {
            return Ok(());
        }
        // `$07:D8A3` places the item relative to the fixed view, then uses it
        // (`$07:DC8B`); the delay byte is rewritten afterwards.
        let _ = CONSUMABLE_REUSE_DELAY;
        Err(PlayerStrategyError::UnportedConsumableUse)
    }

    /// `$06:9DBA..A31C`: post-mode publication, mission admission, node exit,
    /// exit control, layout advance, the stage-exit gate and the retained aim.
    fn advance_player_strategy_tail(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        // The damage-particle number is the frame loop's direct-page $00, a
        // render-paced countdown; the installer faults if it needs one.
        let pacing = self.world.frame_pacing;
        self.advance_player_post_motion(owner, pacing)?;
        if self.advance_player_mission_admission(owner)? == MissionAdmission::ContinueExitControl {
            self.advance_player_node_exit()?;
            self.gate_exit_controller(owner)
                .map_err(|error| self.strategy_error(error))?;
            self.consume_player_layout_advance()?;
            self.gate_stage_exit(owner)
                .map_err(|error| self.strategy_error(error))?;
        }
        self.retain_player_weapon_aim(owner)
    }

    /// `$06:A0A5..A0F8`: with every objective cleared, an unblocked live
    /// player marks its exit transition and installs the exit controller.
    fn gate_exit_controller(&mut self, owner: ObjectId) -> Result<(), PlayerStrategyError> {
        let world = &mut *self.world;
        if world
            .contacts_enabled()
            .ok_or(PlayerStrategyError::MissingObjectiveState)?
        {
            return Ok(());
        }
        if world
            .player_service_flags
            .ok_or(PlayerStrategyError::MissingPlayerServices)?
            .minimum_protection()
        {
            return Ok(());
        }
        if world
            .action_gate
            .ok_or(PlayerStrategyError::MissingActionGate)?
            .code
            != 0
        {
            return Ok(());
        }
        if self
            .objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .hit_points
            == 0
        {
            return Ok(());
        }
        if world
            .view_transition_mode
            .ok_or(PlayerStrategyError::MissingViewMode)?
            .flags
            & EXIT_SUPPRESSED_MODE
            != 0
        {
            return Ok(());
        }
        let records = world.player_mut(self.objects, owner)?;
        if records
            .contact
            .ok_or(WorldInputError::MissingPlayerContact(owner))?
            .ignores_contacts
        {
            return Ok(());
        }
        records
            .mode_selection
            .as_mut()
            .ok_or(PlayerStrategyError::MissingModeSelection(owner))?
            .transition_control |= EXIT_TRANSITION;
        if records
            .action
            .ok_or(PlayerStrategyError::MissingAction(owner))?
            .action
            .is_some()
        {
            return Ok(());
        }
        Err(PlayerStrategyError::UnportedExitController)
    }

    /// `$06:A108..A13C`: the stage-exit gate, after the layout advance.
    fn gate_stage_exit(&mut self, owner: ObjectId) -> Result<(), PlayerStrategyError> {
        let records = self.world.player(self.objects, owner)?;
        let contact = records
            .contact
            .ok_or(WorldInputError::MissingPlayerContact(owner))?;
        if contact.ignores_contacts || contact.hit.hold_secondary_protection {
            return Ok(());
        }
        if !self
            .world
            .contacts_enabled()
            .ok_or(PlayerStrategyError::MissingObjectiveState)?
        {
            return Ok(());
        }
        let handoff = self
            .world
            .handoff
            .as_mut()
            .ok_or(PlayerStrategyError::MissingHandoff)?;
        if handoff.player_flags & HANDOFF_EXIT_ACTIVE == 0 {
            if handoff.player_flags & HANDOFF_EXIT_REQUESTED == 0 {
                return Ok(());
            }
            handoff.player_flags |= HANDOFF_EXIT_ACTIVE;
        }
        Err(PlayerStrategyError::UnportedStageExit)
    }
}

/// `$06:DB23..DBBF`: retain the return position and clear the motion,
/// thrust and control bytes flight entry starts from. Bytes with no native
/// reader (6A70, 6A75, 6A7A..6A7C, 6A7F, 6AA3, 6AAB/C, 6AE0, 6AE4,
/// 6AEB..6AED, 6AEF, 6AF1, 6B05..6B09) have no modeled owner.
fn reset_flight_entry(
    objects: &super::ObjectStore,
    world: &mut super::scene_path_world::ScenePathWorld,
    owner: ObjectId,
) -> Result<(), PlayerStrategyError> {
    let position = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    world.bind_shots(
        objects,
        owner,
        super::path_shots::ActiveShots::from_count(0),
    )?;
    let records = world.player_mut(objects, owner)?;
    let missing = PlayerStrategyError::MissingRecord;
    records
        .boundary
        .as_mut()
        .ok_or(missing("boundary"))?
        .return_position = position;
    // 6BED..6BF1 is also the carried player's origin.
    records.carried.as_mut().ok_or(missing("carried"))?.origin = position;
    let charge = records.charge.as_mut().ok_or(missing("charge"))?;
    charge.rapid_control = 0;
    charge.speed_impulse = 0;
    records
        .consumable
        .as_mut()
        .ok_or(missing("consumable"))?
        .input_control = 0;
    records.roll.as_mut().ok_or(missing("roll"))?.tap_window = 0;
    records.pose.as_mut().ok_or(missing("pose"))?.yaw_offset = 0;
    let motion = records.motion.as_mut().ok_or(missing("motion"))?;
    motion.lateral_impulse = 0;
    motion.walker_attachment_yaw = 0;
    motion.walker_motion_control = 0;
    motion.walker_turn_control = 0;
    motion.walker_stride_control = 0;
    records
        .camera_angles
        .as_mut()
        .ok_or(missing("camera angles"))?
        .height_control = 0;
    records.speed.as_mut().ok_or(missing("speed"))?.thrust = 0;
    records
        .steering
        .as_mut()
        .ok_or(missing("steering"))?
        .direction_age = 0;
    records.flight_displacement = Some(Default::default());
    records
        .surface
        .as_mut()
        .ok_or(missing("surface"))?
        .plane_height = 0;
    records
        .ambient
        .as_mut()
        .ok_or(missing("ambient"))?
        .retained_offset = 0;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PILOT_PAIR_SHAPES;

    #[test]
    fn pilot_pair_shapes_are_the_source_shape_tokens() {
        let tokens = PILOT_PAIR_SHAPES.map(|shape| shape.catalog_entry().unwrap().shape_id);
        assert_eq!(tokens, [0xC24C, 0xC5E8, 0xC94C]);
    }
}
