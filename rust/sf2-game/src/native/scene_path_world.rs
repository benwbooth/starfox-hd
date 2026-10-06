//! Scene-owned inputs for the native path driver.
//!
//! A publication and a live player record are different owners. Selected
//! player borrows are resolved on each statement; crossing predicates use the
//! current object transforms on each candidate. Unpublished services remain
//! absent and retain the path interpreter's explicit missing-input errors.
//! This adapter does not initialize a mission or advance any scene clock.

#[cfg(test)]
#[path = "scene_path_world_tests.rs"]
mod tests;

use super::collision_contacts::ContactStore;
use super::path_control::{forward_plane_projection, PlayerTarget};
use super::path_equipment::SelectedEquipment;
use super::path_invocation::InvocationWorld;
use super::path_motion::{PlayerDisplacement, PublishedPlayerMotion};
use super::path_program::{
    ActionGate, CampaignPathInputs, EncounterSignals, GuidanceHistory, PathWorld, PickupHistory,
    PrimaryMotionInput, ProjectileTrigger, ScenePathInputs, SceneryDistanceState,
    SelectedAuxiliaryState, SelectedParticleEffects,
};
use super::path_radio::{DeferredMessage, PathRadio, RadioEvent, RadioLayout, RadioRequest};
use super::path_runtime::TriggerWorldInputs;
use super::path_scene_state::{
    ActiveNodeFlags, CameraTrackingTarget, EncounterCameraFocus, EncounterCoordination,
    EncounterHandoff, EncounterHealthDisplay, EncounterObjectiveCounts, ObjectiveCompletion,
    PathLatches, SceneEventFlags, SoundBankRequest,
};
use super::path_score::PlayerScore;
use super::path_shots::{ActiveShots, LinkedShotCount, ProjectileFlightOverride};
use super::path_sound::{CueListener, MarkerInputs, PathAudio};
use super::path_trigger_conditions::{ControlledAuxFlags, PlayerPartTarget};
use super::path_triggers::TriggerKind;
use super::platform_carry::CarriedPlayer;
use super::scene_proxy::SceneProxyStore;
use super::{
    Angle, AudioState, ObjectId, ObjectLifetimeId, ObjectSpawnDefaults, ObjectStore, RandomState,
    Rotation, OBJECT_CAPACITY,
};

/// Fields installed by the scene/player initializer and subsequently owned
/// by that player's services. None means not supplied, not a cleared byte.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPathRecords {
    pub contact: Option<super::scene_contact::PlayerContactControl>,
    pub protection: Option<super::path_protection::DeflectionProtection>,
    pub auxiliary: Option<SelectedAuxiliaryState>,
    pub charge: Option<super::player_charge::PlayerCharge>,
    pub equipment: Option<SelectedEquipment>,
    pub score: Option<PlayerScore>,
    pub particles: Option<SelectedParticleEffects>,
    pub controlled_flags: Option<ControlledAuxFlags>,
    pub displacement: Option<PlayerDisplacement>,
    pub carried: Option<CarriedPlayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundPlayer {
    lifetime: ObjectLifetimeId,
    records: PlayerPathRecords,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundShots {
    lifetime: ObjectLifetimeId,
    count: ActiveShots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioRouting {
    pub listeners: [CueListener; 2],
    pub markers: Option<MarkerInputs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldInputError {
    MissingActor(ObjectId),
    StalePlayerRecord(ObjectId),
    MissingSelectedPlayer(PlayerTarget),
    MissingDisplacement(PlayerTarget),
    MissingControlledFlags(PlayerTarget),
    MissingAuxiliary(ObjectId),
    MissingCarryRecords,
    MissingPlayerContact(ObjectId),
    MissingPlayerProtection(ObjectId),
    MissingPlayerParticles(ObjectId),
    MissingContactEnable,
    MissingActionGate,
    MissingPlayerConfiguration,
    MissingPlayerCharge(ObjectId),
}

/// Owning records shared by all actors in a native scene. Scene entry must
/// publish its actual inputs; constructing this container fabricates none.
/// Additional source services can be bound as their canonical owners migrate.
pub struct ScenePathWorld {
    pub random: RandomState,
    pub strategy_clock: u16,
    pub scene: ScenePathInputs,
    pub primary_player: Option<ObjectId>,
    pub secondary_player: Option<ObjectId>,
    pub fixed_players: [Option<ObjectId>; 2],
    /// Shared D7F4 enable, also consumed by the linked protection service.
    pub contacts_enabled: Option<bool>,
    /// Shared 1AA6 bit 02 for reflection-list traversal.
    pub reflect_all_contacts: Option<bool>,
    pub weapons: Option<super::weapon_dispatch::WeaponState>,
    players: [Option<BoundPlayer>; OBJECT_CAPACITY],
    // Separate borrows of selected equipment and linked shot counts can name
    // the same player. Both stores have one generation-checked owner.
    shots: [Option<BoundShots>; OBJECT_CAPACITY],
    pub audio: AudioState,
    pub audio_routing: Option<AudioRouting>,
    pub radio: Option<(RadioRequest, RadioLayout)>,
    pub contacts: ContactStore,
    pub proxies: SceneProxyStore,
    pub campaign: Option<CampaignPathInputs>,
    pub camera_heading: Option<Angle>,
    pub spawn_defaults: Option<ObjectSpawnDefaults>,
    pub published_motion: Option<PublishedPlayerMotion>,
    pub active_charge_threshold: Option<u8>,
    pub handoff: Option<EncounterHandoff>,
    pub camera_focus: Option<EncounterCameraFocus>,
    pub camera_tracking: Option<CameraTrackingTarget>,
    pub health_display: Option<EncounterHealthDisplay>,
    pub coordination: Option<EncounterCoordination>,
    pub objective_counts: Option<EncounterObjectiveCounts>,
    pub objective_completion: Option<ObjectiveCompletion>,
    pub scene_events: Option<SceneEventFlags>,
    pub path_latches: Option<PathLatches>,
    pub sound_bank_request: Option<SoundBankRequest>,
    pub encounter_signals: Option<EncounterSignals>,
    pub scenery_distance: Option<SceneryDistanceState>,
    pub action_gate: Option<ActionGate>,
    pub projectile_trigger: Option<ProjectileTrigger>,
    pub projectile_flight_override: Option<ProjectileFlightOverride>,
    pub deferred_message: Option<DeferredMessage>,
    pub radio_event: Option<RadioEvent>,
    pub guidance: Option<GuidanceHistory>,
    pub pickup_history: Option<PickupHistory>,
    pub active_node_flags: Option<ActiveNodeFlags>,
}

impl ScenePathWorld {
    pub fn new(random: RandomState) -> Self {
        Self {
            random,
            strategy_clock: 0,
            scene: ScenePathInputs::default(),
            primary_player: None,
            secondary_player: None,
            fixed_players: [None; 2],
            contacts_enabled: None,
            reflect_all_contacts: None,
            weapons: None,
            players: [None; OBJECT_CAPACITY],
            shots: [None; OBJECT_CAPACITY],
            audio: AudioState::default(),
            audio_routing: None,
            radio: None,
            contacts: ContactStore::default(),
            proxies: SceneProxyStore::default(),
            campaign: None,
            camera_heading: None,
            spawn_defaults: None,
            published_motion: None,
            active_charge_threshold: None,
            handoff: None,
            camera_focus: None,
            camera_tracking: None,
            health_display: None,
            coordination: None,
            objective_counts: None,
            objective_completion: None,
            scene_events: None,
            path_latches: None,
            sound_bank_request: None,
            encounter_signals: None,
            scenery_distance: None,
            action_gate: None,
            projectile_trigger: None,
            projectile_flight_override: None,
            deferred_message: None,
            radio_event: None,
            guidance: None,
            pickup_history: None,
            active_node_flags: None,
        }
    }

    pub fn bind_player(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        records: PlayerPathRecords,
    ) -> Result<(), WorldInputError> {
        let lifetime = objects
            .lifetime_id(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        self.players[owner.index()] = Some(BoundPlayer { lifetime, records });
        Ok(())
    }

    pub fn player_mut(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
    ) -> Result<&mut PlayerPathRecords, WorldInputError> {
        let binding = self.players[owner.index()]
            .as_mut()
            .filter(|binding| Some(binding.lifetime) == objects.lifetime_id(owner))
            .ok_or(WorldInputError::StalePlayerRecord(owner))?;
        Ok(&mut binding.records)
    }

    pub fn player(
        &self,
        objects: &ObjectStore,
        owner: ObjectId,
    ) -> Result<&PlayerPathRecords, WorldInputError> {
        let binding = self.players[owner.index()]
            .as_ref()
            .filter(|binding| Some(binding.lifetime) == objects.lifetime_id(owner))
            .ok_or(WorldInputError::StalePlayerRecord(owner))?;
        Ok(&binding.records)
    }

    pub fn bind_shots(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        count: ActiveShots,
    ) -> Result<(), WorldInputError> {
        let lifetime = objects
            .lifetime_id(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        self.shots[owner.index()] = Some(BoundShots { lifetime, count });
        Ok(())
    }

    pub fn shots(&self, objects: &ObjectStore, owner: ObjectId) -> Option<ActiveShots> {
        self.shots[owner.index()]
            .filter(|binding| Some(binding.lifetime) == objects.lifetime_id(owner))
            .map(|binding| binding.count)
    }

    fn selected(&self, selected: PlayerTarget) -> Option<ObjectId> {
        match selected {
            PlayerTarget::Primary => self.primary_player,
            PlayerTarget::Secondary => self.secondary_player,
        }
    }
}

impl InvocationWorld for ScenePathWorld {
    type Error = WorldInputError;

    fn path_world(
        &mut self,
        objects: &ObjectStore,
        actor: ObjectId,
        selected: PlayerTarget,
    ) -> Result<PathWorld<'_>, Self::Error> {
        let actor = objects
            .get(actor)
            .ok_or(WorldInputError::MissingActor(actor))?;
        let selected = self.selected(selected);
        let primary_motion = self
            .primary_player
            .and_then(|id| self.players[id.index()])
            .filter(|binding| {
                Some(binding.lifetime) == objects.lifetime_id(binding.lifetime.slot())
            })
            .and_then(|binding| {
                Some(PrimaryMotionInput {
                    auxiliary_mode: binding.records.auxiliary?.mode,
                    displacement: binding.records.displacement?.world_delta,
                })
            });
        let linked_shots = actor
            .base
            .attachment
            .and_then(|id| self.shots[id.index()].as_mut())
            .filter(|binding| {
                Some(binding.lifetime) == objects.lifetime_id(binding.lifetime.slot())
            });
        let mut world = PathWorld::unbound(&mut self.random, self.strategy_clock as u8);
        world.scene = self.scene;
        world.primary_player = self.primary_player;
        world.secondary_player = self.secondary_player;
        world.fixed_players = self.fixed_players;
        world.selected = selected;
        world.primary_motion = primary_motion;
        world.published_motion = self.published_motion;
        world.active_charge_threshold = self.active_charge_threshold;
        // One traversal can borrow disjoint fields even when selected and
        // primary identify the same player. No cloned hit-state copyback.
        for player in self.players.iter_mut().flatten() {
            let owner = player.lifetime.slot();
            if Some(player.lifetime) != objects.lifetime_id(owner) {
                continue;
            }
            let records = &mut player.records;
            if selected == Some(owner) {
                world.selected_auxiliary = records.auxiliary.as_mut();
                world.selected_charge = records.charge.map(|charge| charge.path_input());
                world.selected_equipment = records.equipment.as_mut();
                world.selected_score = records.score.as_mut();
                world.selected_particle_effects = records.particles.as_mut();
            }
            if self.primary_player == Some(owner) {
                world.primary_feedback = records.contact.as_mut().map(|contact| {
                    super::player_hit_control::PrimaryFeedback {
                        state: contact.hit.reserve_shield,
                        hit: &mut contact.hit,
                    }
                });
            }
        }
        world.linked_shot_count = linked_shots.map(|binding| LinkedShotCount {
            owner: binding.lifetime.slot(),
            state: &mut binding.count,
        });
        world.audio = self.audio_routing.map(|routing| PathAudio {
            events: &mut self.audio,
            listeners: routing.listeners,
            markers: routing.markers,
        });
        world.radio = self.radio.as_mut().map(|(request, layout)| PathRadio {
            request,
            layout: *layout,
        });
        world.contacts = Some(&self.contacts);
        world.scene_proxies = Some(&mut self.proxies);
        world.campaign = self.campaign;
        world.camera_heading = self.camera_heading;
        world.spawn_defaults = self.spawn_defaults;
        world.handoff = self.handoff.as_mut();
        world.camera_focus = self.camera_focus.as_mut();
        world.camera_tracking = self.camera_tracking.as_mut();
        world.health_display = self.health_display.as_mut();
        world.coordination = self.coordination.as_mut();
        world.objective_counts = self.objective_counts.as_mut();
        world.objective_completion = self.objective_completion.as_mut();
        world.scene_events = self.scene_events.as_mut();
        world.path_latches = self.path_latches.as_mut();
        world.sound_bank_request = self.sound_bank_request.as_mut();
        world.encounter_signals = self.encounter_signals.as_mut();
        world.scenery_distance = self.scenery_distance.as_mut();
        world.action_gate = self.action_gate.as_mut();
        world.projectile_trigger = self.projectile_trigger.as_mut();
        world.projectile_flight_override = self.projectile_flight_override.as_mut();
        world.deferred_message = self.deferred_message.as_mut();
        world.radio_event = self.radio_event.as_mut();
        world.guidance = self.guidance.as_mut();
        world.pickup_history = self.pickup_history.as_mut();
        world.active_node_flags = self.active_node_flags.as_mut();
        world.weapons = self.weapons.as_mut();
        Ok(world)
    }

    fn displacement(
        &mut self,
        objects: &ObjectStore,
        selected: PlayerTarget,
    ) -> Result<PlayerDisplacement, Self::Error> {
        let owner = self
            .selected(selected)
            .ok_or(WorldInputError::MissingSelectedPlayer(selected))?;
        self.player_mut(objects, owner)?
            .displacement
            .ok_or(WorldInputError::MissingDisplacement(selected))
    }

    fn trigger_inputs(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        selected: PlayerTarget,
        kind: TriggerKind,
    ) -> Result<TriggerWorldInputs, Self::Error> {
        let mut inputs = TriggerWorldInputs {
            strategy_tick: self.strategy_clock as u8,
            ..TriggerWorldInputs::default()
        };
        match kind {
            TriggerKind::PlayerCrossing => {
                let actor = objects
                    .get(owner)
                    .ok_or(WorldInputError::MissingActor(owner))?;
                let rotation = Rotation {
                    pitch: actor.base.pitch,
                    yaw: actor.base.yaw,
                    roll: actor.base.roll,
                };
                let mut crossing = actor.extension.path_state.conditions.crossing;
                for (index, player) in [self.primary_player, self.secondary_player]
                    .into_iter()
                    .enumerate()
                {
                    if let Some(player) = player {
                        let target = objects
                            .get(player)
                            .ok_or(WorldInputError::MissingActor(player))?;
                        inputs.player_projections[index] = Some(forward_plane_projection(
                            actor.base.position,
                            rotation,
                            target.base.position,
                        ));
                        // An already initialized primary crossing exits before
                        // reading the secondary transform. On initialization
                        // both signs must be sampled before comparison.
                        if index == 0
                            && crossing.sample(inputs.player_projections)
                                == Some(PlayerTarget::Primary)
                        {
                            break;
                        }
                    }
                }
            }
            TriggerKind::ControlledAuxFlagHigh | TriggerKind::ControlledAuxFlagLow => {
                let owner = self
                    .selected(selected)
                    .ok_or(WorldInputError::MissingSelectedPlayer(selected))?;
                inputs.controlled_aux = self
                    .player_mut(objects, owner)?
                    .controlled_flags
                    .ok_or(WorldInputError::MissingControlledFlags(selected))?;
            }
            TriggerKind::PlayerPartTarget => {
                let part = objects
                    .get(owner)
                    .ok_or(WorldInputError::MissingActor(owner))?
                    .extension
                    .surface_contact
                    .group;
                for (index, player) in [self.primary_player, self.secondary_player]
                    .into_iter()
                    .enumerate()
                {
                    let Some(player) = player else {
                        continue;
                    };
                    let actor = objects
                        .get(player)
                        .ok_or(WorldInputError::MissingActor(player))?;
                    let mode = self
                        .player_mut(objects, player)?
                        .auxiliary
                        .ok_or(WorldInputError::MissingAuxiliary(player))?
                        .mode;
                    let target = PlayerPartTarget {
                        part_target_mode: mode & 0xF0 == 0x20,
                        actor: actor.extension.surface_contact.supporting_object,
                        part: actor.extension.surface_contact.group,
                        enabled: actor.base.flags.standing_on_surface,
                    };
                    inputs.player_parts[index] = Some(target);
                    if index == 0
                        && target.part_target_mode
                        && target.actor == Some(owner)
                        && target.part == part
                        && target.enabled
                    {
                        break;
                    }
                }
            }
            TriggerKind::Always
            | TriggerKind::Periodic(_)
            | TriggerKind::NewContact
            | TriggerKind::PlayerContact
            | TriggerKind::ConsumeHitEvent
            | TriggerKind::Detached
            | TriggerKind::ZeroHealth
            | TriggerKind::TimerPenultimate => {}
        }
        Ok(inputs)
    }

    fn carried_player(
        &mut self,
        objects: &ObjectStore,
        carrier: ObjectId,
        selected: PlayerTarget,
    ) -> Result<Option<&mut CarriedPlayer>, Self::Error> {
        let Some(owner) = self.selected(selected) else {
            return Ok(None);
        };
        let actor = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        if !actor.base.flags.standing_on_surface
            || actor.extension.surface_contact.supporting_object != Some(carrier)
        {
            return Ok(None);
        }
        let player = self
            .player_mut(objects, owner)?
            .carried
            .as_mut()
            .ok_or(WorldInputError::MissingCarryRecords)?;
        // These are fresh invocation observations; auxiliary origin/fine yaw
        // remain the actual mutable player-owned records.
        player.enabled = true;
        player.carrier = Some(carrier);
        Ok(Some(player))
    }
}
