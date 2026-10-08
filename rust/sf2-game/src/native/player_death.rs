//! Scene-player death routines. `$06:F3A4..F511` is the type-12 auxiliary
//! record that replaces common destruction for a scene player: the player is
//! not retired but gains a smoke child, installs the defeat action
//! (`$0D:C585`) and a second routine, `$06:F512..F895`. With zero health the
//! strategy selection runs that second routine instead of the player
//! strategy on every later visit: it runs the action, falls, explodes at
//! countdown 50 (a blast actor, `$06:80C1`, and a colour-cycling sprite) and
//! leaves an empty shape.

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
    SoundEvent, Vector3, OBJECT_CAPACITY,
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
/// `$06:F51F`: the countdown stops at its maximum.
const COUNTDOWN_LIMIT: u16 = 0xFFFF;
/// Countdown values selecting the follow-up routine's stages.
const RETARGET_AT: i16 = 0x25;
const HOLD_AT: i16 = 0x26;
const FLASH_FROM: i16 = 0x28;
const MARK_FROM: i16 = 0x2E;
const PREBURST_LIMIT: i16 = 0x31;
const BURST_AT: i16 = 0x32;
/// `$06:F5CA` / `$06:F609`: the two target configurations.
const RETARGET_CONTROL: (i16, u8, u8, [u8; 3], [u8; 3]) = (0, 3, 1, [31; 3], [4, 8, 8]);
const BURST_CONTROL: (i16, u8, u8, [u8; 3], [u8; 3]) = (2, 3, 0, [31; 3], [0, 2, 2]);
/// `$06:F652`: the numbered child retired on every visit.
const RETIRED_CHILD: u8 = 0x16;
const WALKER_FAMILY: u8 = 0x20;
const ALTERNATE_FAMILY: u8 = 0x30;
/// `$06:F78B`: the falling roll step.
const FALL_ROLL_STEP: i8 = 5;
/// `$06:F726..F763`: the blast actor's fields.
const BLAST_WAIT: u8 = 6;
const BLAST_CHILD_NUMBER: u8 = 0x3C;
const BLAST_REPEAT: u8 = 0x7F;
/// `$06:F7E0`: the colour-cycling sprite's health and attack.
const SPRITE_HEALTH: u8 = 10;
const SPRITE_ATTACK: u8 = 10;
/// ShapeHdr BE08.
const BURST_SPRITE_SHAPE: ShapeId = ShapeId::from_catalog_index(13);
/// `$06:F7F1`: the burst cue.
const BURST_CUE: u8 = 0x10;
/// `$06:80D8`/`$06:80FE`: the blast actor's counter thresholds.
const BLAST_SETTLE: u8 = 0x64;
const BLAST_RETIRE: u8 = 0x7F;

/// The blast actor's strategy (`$06:80C1` installs `$06:80D8`, which hands
/// over to `$06:80F5` once its counter reaches 100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefeatBlastPhase {
    Install,
    Rise,
    Settle,
}

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
    MissingOccupancy,
    MissingOccupancyExemption(ObjectId),
    MissingProcessedInput,
    Action(super::player_action::PlayerActionError),
    Palette(super::player_palette::PaletteError),
    Reticle(super::player_reticle::ReticleError),
    Status(super::player_status::StatusError),
    Camera(super::player_camera_dispatch::CameraDispatchError),
    Blend(super::view_blend::ViewBlendError),
    /// `$06:F682`: the Walker's defeat branch (`$06:D781`) is not ported.
    UnportedWalkerDefeat,
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
            DeathHandler::DefeatedScenePlayer => self.continue_defeat(owner),
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
        control.set_axis_rates(DEFEAT_AXIS_RATES);
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

impl<C: SceneCallbacks> SceneActors<'_, C> {
    /// `$06:F512..F824`, run in place of the player strategy each visit.
    fn continue_defeat(&mut self, owner: ObjectId) -> Result<(), PlayerDeathError> {
        let objects = &mut *self.objects;
        let world = &mut *self.world;
        let runtime = &mut self.execution.paths.runtime;
        world
            .player_mut(objects, owner)?
            .contact
            .as_mut()
            .ok_or(WorldInputError::MissingPlayerContact(owner))?
            .hit
            .reserve_shield = 0;
        let actor = objects
            .get_mut(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        let countdown = &mut actor.extension.path_state.script_value;
        if *countdown != COUNTDOWN_LIMIT {
            *countdown += 1;
        }
        let input = world
            .processed_player_input
            .ok_or(PlayerDeathError::MissingProcessedInput)?;
        super::player_action::advance(objects, world, &mut runtime.resources, owner, input)
            .map_err(PlayerDeathError::Action)?;
        objects
            .get_mut(owner)
            .expect("validated player")
            .base
            .flags
            .view_side_filter = false;
        let exempt = world
            .player(objects, owner)?
            .occupancy_exempt
            .ok_or(PlayerDeathError::MissingOccupancyExemption(owner))?;
        let stage = |objects: &super::ObjectStore| {
            objects.get(owner).expect("validated player").extension.path_state.script_value as i16
        };
        if !exempt {
            let position = objects.get(owner).expect("validated player").base.position;
            let landed = world
                .occupancy
                .as_ref()
                .ok_or(PlayerDeathError::MissingOccupancy)?
                .contains(position);
            if landed && stage(objects) <= BURST_AT {
                objects.get_mut(owner).expect("validated player").extension.path_state.script_value =
                    BURST_AT as u16;
            }
        }
        let primary = world.primary_player.ok_or(PlayerDeathError::MissingPrimaryPlayer)?;
        let position = objects.get(owner).expect("validated player").base.position;
        let current = stage(objects);
        let configuration = if current == RETARGET_AT {
            Some(RETARGET_CONTROL)
        } else if current == BURST_AT {
            Some(BURST_CONTROL)
        } else {
            None
        };
        let refresh = current == RETARGET_AT || current == BURST_AT || current == HOLD_AT || current > BURST_AT;
        if current > BURST_AT && current != HOLD_AT && current != RETARGET_AT {
            super::player_palette::flash(world).map_err(PlayerDeathError::Palette)?;
        }
        if refresh {
            let control = world
                .player_mut(objects, primary)?
                .target_control
                .as_mut()
                .ok_or(PlayerDeathError::MissingTargetControl(primary))?;
            if let Some((range, axis_mode, control_bits, limits, rates)) = configuration {
                // `$06:F609` unlocks first; `$06:F5CA` does not.
                if current == BURST_AT {
                    control.configuration_locked = false;
                }
                control.configure_origin(owner, position, range, axis_mode, control_bits);
                control.set_axis_limits(limits);
                control.set_axis_rates(rates);
                control.configuration_locked = true;
            }
            control.refresh_owned_origin(owner, position);
        }
        if let Some(child) = path_relationships::find_direct_child(objects, owner, RETIRED_CHILD)
            .map_err(PlayerDeathError::Relationships)?
        {
            objects.get_mut(child).expect("found child").base.flags.remove_after_tick = true;
        }
        let mode = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(PlayerDeathError::MissingAuxiliary(owner))?
            .mode;
        if mode & FAMILY_MASK == WALKER_FAMILY {
            return Err(PlayerDeathError::UnportedWalkerDefeat);
        }
        let actor = objects.get_mut(owner).expect("validated player");
        actor.base.pitch = Angle::from_units(super::path_fields::chase_byte(actor.base.pitch.units(), 0));
        super::player_reticle::prepare(objects, world, owner).map_err(PlayerDeathError::Reticle)?;
        super::player_status::advance_transform_cues(objects, world, owner)
            .map_err(PlayerDeathError::Status)?;
        super::player_camera_dispatch::advance(objects, world, runtime, owner)
            .map_err(PlayerDeathError::Camera)?;
        super::view_blend::advance(objects, world).map_err(PlayerDeathError::Blend)?;
        super::player_status::advance_shield(objects, world, &mut runtime.resources, owner)
            .map_err(PlayerDeathError::Status)?;
        let current = stage(objects);
        if current <= BURST_AT && current >= FLASH_FROM && current >= MARK_FROM && world.strategy_clock & 1 == 0 {
            objects.get_mut(owner).expect("validated player").base.contacts.hit_marked = true;
        }
        if current == BURST_AT && objects.len() < OBJECT_CAPACITY {
            let defaults = world
                .spawn_defaults()
                .ok_or(PlayerDeathError::MissingSpawnDefaults)?;
            let mut blast = Object::new_authored(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::DefeatBlast(DefeatBlastPhase::Install),
                defaults,
            );
            blast.base.hit_points = 1;
            blast.base.attack_power = 1;
            blast.base.flags.collision_disabled = true;
            blast.base.position = position;
            blast.base.wait_timer = BLAST_WAIT;
            blast.base.child_number = BLAST_CHILD_NUMBER;
            blast.extension.path_state.repeat_counter = BLAST_REPEAT;
            let head = objects.active_ids().first().copied();
            objects.allocate_after(head, blast).expect("a free slot was checked");
        }
        if mode & FAMILY_MASK != WALKER_FAMILY && mode & FAMILY_MASK != ALTERNATE_FAMILY {
            let actor = objects.get_mut(owner).expect("validated player");
            actor.base.roll = actor.base.roll.wrapping_add(FALL_ROLL_STEP);
            let velocity = actor.base.velocity;
            actor.base.position = Vector3 {
                x: actor.base.position.x.wrapping_add(velocity.x),
                y: actor.base.position.y.wrapping_add(velocity.y),
                z: actor.base.position.z.wrapping_add(velocity.z),
            };
        }
        if current == BURST_AT {
            let actor = objects.get_mut(owner).expect("validated player");
            actor.base.velocity = Vector3::default();
            let position = actor.base.position;
            if objects.len() < OBJECT_CAPACITY {
                let defaults = world
                    .spawn_defaults()
                    .ok_or(PlayerDeathError::MissingSpawnDefaults)?;
                let mut sprite = Object::new_authored(
                    ObjectKind::Effect,
                    BURST_SPRITE_SHAPE,
                    Behavior::FollowPath,
                    defaults,
                );
                sprite.extension.path_state.needs_path_initialization = true;
                sprite.base.path = Some(authored_paths::COLOR_CYCLE_SPRITE);
                sprite.base.hit_points = SPRITE_HEALTH;
                sprite.base.attack_power = SPRITE_ATTACK;
                sprite.base.position = position;
                let head = objects.active_ids().first().copied();
                objects.allocate_after(head, sprite).expect("a free slot was checked");
            }
            let target = if owner == primary {
                PlayerTarget::Primary
            } else {
                PlayerTarget::Secondary
            };
            world.audio.queue(SoundEvent::Authored(AuthoredCue::new(BURST_CUE, 0, target)));
            // `$06:F825` re-targets the Walker leg parts (6AEF/6AF1); only the
            // Walker phase installs them and flight entry clears both words.
            objects.get_mut(owner).expect("validated player").base.shape = ShapeId::EMPTY;
        }
        if stage(objects) <= PREBURST_LIMIT {
            attachments::refresh_child_chain(objects, owner)
                .map_err(PlayerDeathError::Attachments)?;
        }
        Ok(())
    }
}

/// One visit of the blast actor (`$06:80C1..810E`). Its counter is the
/// shared base byte 0A; it follows the primary player and retires at 127.
pub fn step_defeat_blast(
    objects: &mut super::ObjectStore,
    primary: Option<ObjectId>,
    owner: ObjectId,
) -> Result<(), PlayerDeathError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let Behavior::DefeatBlast(mut phase) = actor.base.behavior else {
        return Err(WorldInputError::MissingActor(owner).into());
    };
    if phase == DefeatBlastPhase::Install {
        actor.base.contacts.suppress_contacts_next_epoch = true;
        actor.base.target_speed = 0;
        phase = DefeatBlastPhase::Rise;
    }
    if phase == DefeatBlastPhase::Rise && actor.base.target_speed as i8 >= BLAST_SETTLE as i8 {
        phase = DefeatBlastPhase::Settle;
        actor.base.wait_timer = 0;
    }
    actor.base.behavior = Behavior::DefeatBlast(phase);
    actor.base.target_speed = actor.base.target_speed.wrapping_add(1);
    let primary = primary.ok_or(PlayerDeathError::MissingPrimaryPlayer)?;
    let position = objects
        .get(primary)
        .ok_or(WorldInputError::MissingActor(primary))?
        .base
        .position;
    let actor = objects.get_mut(owner).expect("validated blast");
    actor.base.position = position;
    if actor.base.target_speed as i8 >= BLAST_RETIRE as i8 {
        actor.base.flags.remove_after_tick = true;
    }
    Ok(())
}
