//! Scene-player death routine (`$06:F3A4..F511`), the type-12 auxiliary
//! record that replaces common destruction for a scene player. The player is
//! not retired: it keeps its flight strategy, gains a smoke child, installs
//! the defeat action (`$0D:C585`) and a second death routine (`$06:F512`).

use super::actor_auxiliary::{AuxiliaryError, AuxiliaryRecord, DeathHandler};
use super::attachments::{self, AttachmentError};
use super::path_control::PlayerTarget;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::AuthoredCue;
use super::player_action::PlayerAction;
use super::player_engine_sound::EngineSoundControl;
use super::player_throttle::{self, ThrottleError};
use super::scene_path_world::WorldInputError;
use super::scene_strategy::{SceneActors, SceneCallbacks, SceneError};
use super::{
    authored_paths, path_motion, Angle, Behavior, Object, ObjectId, ObjectKind, ShapeId,
    SoundEvent, OBJECT_CAPACITY,
};

/// `$06:F3FB..F402`: the smoke child's health, attack and speed target.
const SMOKE_HEALTH: u8 = 1;
const SMOKE_ATTACK: u8 = 1;
const SMOKE_TARGET_SPEED: u8 = 0x32;
/// `$07:B89B`: the primary player's target-control rates.
const DEFEAT_AXIS_RATES: [u8; 3] = [1, 1, 1];
/// `$06:F45D`/`$06:F472`: flight-family and other defeat cues.
const FLIGHT_FAMILY: u8 = 0x10;
const FAMILY_MASK: u8 = 0xF0;
const FLIGHT_DEFEAT_CUE: u8 = 0x11;
const GROUND_DEFEAT_CUE: u8 = 0x15;
/// `$06:F483`: the falling speed, with the pitch input (base 13) cleared.
const DEFEAT_SPEED: u8 = 0x1E;
const VELOCITY_SCALE: i16 = 1;
/// `$06:F4C2`: the working-word countdown unless 1AA6 bit 02 is set.
const DEFEAT_COUNTDOWN: u16 = 0x1E;
/// `$06:F4E4`: Walker stride bits retained.
const STRIDE_RETAINED: u8 = 0x1F;
/// `$06:F504`: depth word.
const DEFEAT_DEPTH: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerDeathError {
    World(WorldInputError),
    Throttle(ThrottleError),
    Relationships(RelationshipError),
    Attachments(AttachmentError),
    Auxiliary(AuxiliaryError),
    Filters,
    /// The smoke child's number is the frame loop's direct-page $00.
    MissingFramePacing,
    MissingSpawnDefaults,
    MissingPrimaryPlayer,
    MissingTargetControl(ObjectId),
    MissingPlayerServices,
    MissingAction(ObjectId),
    MissingAuxiliary(ObjectId),
    MissingMotion(ObjectId),
    MissingModeFlags,
    MissingFixedView,
}

impl From<WorldInputError> for PlayerDeathError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

impl<C: SceneCallbacks> SceneActors<'_, C> {
    /// Run the actor's registered type-12 routine (`$03:A055`). The routine
    /// returns to the caller of common destruction with the same actor.
    pub(super) fn run_registered_death(
        &mut self,
        owner: ObjectId,
        handler: DeathHandler,
    ) -> Result<ObjectId, SceneError<C::Error>> {
        let result = match handler {
            DeathHandler::ScenePlayer => self.defeat_scene_player(owner),
            DeathHandler::DefeatedScenePlayer => {
                self.execution.faulted = true;
                return Err(SceneError::UnportedDeathHandler(handler));
            }
        };
        if let Err(error) = result {
            self.execution.faulted = true;
            return Err(SceneError::PlayerDeath(error));
        }
        Ok(owner)
    }

    /// `$06:F3A4..F511`.
    fn defeat_scene_player(&mut self, owner: ObjectId) -> Result<(), PlayerDeathError> {
        let objects = &mut *self.objects;
        let world = &mut *self.world;
        player_throttle::cancel(objects, world, owner).map_err(PlayerDeathError::Throttle)?;
        // `$7F:2A17` silently yields no actor when the pool is full.
        if objects.len() < OBJECT_CAPACITY {
            let defaults = world
                .spawn_defaults()
                .ok_or(PlayerDeathError::MissingSpawnDefaults)?;
            let number = world.frame_pacing.ok_or(PlayerDeathError::MissingFramePacing)?;
            let fresh = Object::new_authored(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::FollowPath,
                defaults,
            );
            let head = objects.active_ids().first().copied();
            let smoke = objects
                .allocate_after(head, fresh)
                .expect("a free slot was checked");
            path_relationships::attach_fresh_child(objects, owner, smoke, number)
                .map_err(PlayerDeathError::Relationships)?;
            let smoke = objects.get_mut(smoke).expect("fresh smoke child");
            smoke.extension.path_state.needs_path_initialization = true;
            smoke.base.path = Some(authored_paths::REPEATED_CHILD_SPRITE);
            smoke.base.hit_points = SMOKE_HEALTH;
            smoke.base.attack_power = SMOKE_ATTACK;
            smoke.base.target_speed = SMOKE_TARGET_SPEED;
        }
        let primary = world.primary_player.ok_or(PlayerDeathError::MissingPrimaryPlayer)?;
        let control = world
            .player_mut(objects, primary)?
            .target_control
            .as_mut()
            .ok_or(PlayerDeathError::MissingTargetControl(primary))?;
        if !control.configuration_locked {
            control.axis_rates = DEFEAT_AXIS_RATES;
        }
        world
            .player_service_flags
            .as_mut()
            .ok_or(PlayerDeathError::MissingPlayerServices)?
            .request_minimum_protection();
        world.engine_sound_control = Some(EngineSoundControl::from_bits(0));
        world
            .player_mut(objects, owner)?
            .action
            .as_mut()
            .ok_or(PlayerDeathError::MissingAction(owner))?
            .install(PlayerAction::Defeat);
        let mode = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(PlayerDeathError::MissingAuxiliary(owner))?
            .mode;
        let cue = if mode & FAMILY_MASK == FLIGHT_FAMILY {
            FLIGHT_DEFEAT_CUE
        } else {
            GROUND_DEFEAT_CUE
        };
        let target = if owner == primary {
            PlayerTarget::Primary
        } else {
            PlayerTarget::Secondary
        };
        world.audio.queue(SoundEvent::Authored(AuthoredCue::new(cue, 0, target)));
        // `$7F:2D1F` reads the cleared base 13 as its pitch input.
        let actor = objects
            .get_mut(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        actor.base.speed = DEFEAT_SPEED;
        actor.base.child_number = 0;
        actor.base.velocity =
            path_motion::direction_velocity(Angle::ZERO, actor.base.yaw, DEFEAT_SPEED, VELOCITY_SCALE);
        super::player_status::advance_filters(objects, world, owner)
            .map_err(|_| PlayerDeathError::Filters)?;
        let countdown = if world
            .reflect_all_contacts
            .ok_or(PlayerDeathError::MissingModeFlags)?
        {
            0
        } else {
            DEFEAT_COUNTDOWN
        };
        let actor = objects
            .get_mut(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        actor.extension.path_state.script_value = countdown;
        let mut auxiliary = std::mem::take(&mut actor.extension.auxiliary);
        let registered = auxiliary.set(
            &mut self.execution.paths.runtime.resources,
            owner,
            AuxiliaryRecord::DeathHandler(DeathHandler::DefeatedScenePlayer),
        );
        objects
            .get_mut(owner)
            .expect("validated player")
            .extension
            .auxiliary = auxiliary;
        registered.map_err(PlayerDeathError::Auxiliary)?;
        world
            .player_mut(objects, owner)?
            .motion
            .as_mut()
            .ok_or(PlayerDeathError::MissingMotion(owner))?
            .walker_stride_control &= STRIDE_RETAINED;
        let view = world.fixed_players[0].ok_or(PlayerDeathError::MissingFixedView)?;
        let motion = &mut objects
            .get_mut(view)
            .ok_or(WorldInputError::MissingActor(view))?
            .extension
            .path_state
            .motion;
        motion.follow_player_displacement = true;
        motion.generate_velocity_each_step = true;
        attachments::refresh_child_chain(objects, owner).map_err(PlayerDeathError::Attachments)?;
        objects
            .get_mut(owner)
            .expect("validated player")
            .extension
            .depth_offset = DEFEAT_DEPTH;
        world
            .player_mut(objects, owner)?
            .contact
            .as_mut()
            .ok_or(WorldInputError::MissingPlayerContact(owner))?
            .hit
            .reserve_shield = 0;
        Ok(())
    }
}
