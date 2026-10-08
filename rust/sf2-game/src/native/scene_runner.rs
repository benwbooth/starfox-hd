//! Native per-frame owner for an installed scene, in the original frame
//! order (`$03:8040..8160`): collision queue and deferred retirement
//! (`$7F:32A1`), primary palette (`$07:EA67`), background scroll
//! (`$03:B0C3`), then the strategy epoch (`$7F:34E7`/`$7F:354A`).
//! Frame services that are not ported yet are not assumed to be no-ops; the
//! runner is only used where the differential tests show they have no effect.

use super::collision_pass::{CollisionError, CollisionQueue};
use super::frame_background::{self, BackgroundScrollError};
use super::path_program::PathCatalog;
use super::path_sound::{CueListener, CueMarker, MarkerInputs};
use super::path_control::PlayerTarget;
use super::positional_audio::LoopListener;
use super::scene_path_world::{AudioRouting, ScenePathWorld};
use super::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use super::strategy_schedule::{ScheduleError, StrategyCompletion, StrategyHost, StrategySchedule};
use super::view_transition::FixedViewAngles;
use super::{ObjectId, ObjectStore};

const STATEMENT_BUDGET: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneRunError<E> {
    Scene(SceneError<E>),
    Schedule(ScheduleError<SceneError<E>>),
    Collision(CollisionError),
    Background(BackgroundScrollError),
    MissingFixedView,
    /// Shared scene flag 1B96 bit 0100, which also gates the collision queue.
    MissingQueueGate,
    MissingSpawnDefaults,
    MissingSecondaryMarkerPolicy,
    InvalidRefreshSchedule,
}

impl<E> From<SceneError<E>> for SceneRunError<E> {
    fn from(error: SceneError<E>) -> Self {
        Self::Scene(error)
    }
}

/// Where the frame's entropy refreshes (`$7F:058F`, one extra random draw
/// each) fall among the strategy pass's random draws. The source positions
/// depend on render timing and can fall inside an actor's visit; a long
/// frame can contain more than one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyRefresh<'a> {
    /// Before the pass's consumer draws with these zero-based indices.
    BeforeDraws(&'a [u16]),
    /// One refresh after the pass.
    AfterPass,
}

pub struct SceneRunner<C: SceneCallbacks> {
    pub objects: ObjectStore,
    pub world: ScenePathWorld,
    pub execution: SceneExecution,
    pub schedule: StrategySchedule,
    pub callbacks: C,
}

impl<C: SceneCallbacks> SceneRunner<C> {
    pub fn new(objects: ObjectStore, world: ScenePathWorld, callbacks: C) -> Self {
        Self {
            objects,
            world,
            execution: SceneExecution::default(),
            schedule: StrategySchedule::default(),
            callbacks,
        }
    }

    /// One game frame. A failure latches the scene; nothing is retried.
    pub fn tick(
        &mut self,
        catalog: &PathCatalog,
        refresh: EntropyRefresh<'_>,
    ) -> Result<(), SceneRunError<C::Error>> {
        self.prepare_frame(catalog)?;
        self.run_epoch(catalog, refresh)
    }

    /// The frame services before the strategy epoch: collision queue and
    /// deferred retirement (`$7F:32A1`), primary palette, background scroll.
    pub fn prepare_frame(&mut self, catalog: &PathCatalog) -> Result<(), SceneRunError<C::Error>> {
        let queue_disabled = self
            .world
            .reticle_inhibited
            .ok_or(SceneRunError::MissingQueueGate)?;
        if self.world.fixed_players[0].is_none() {
            return Err(SceneRunError::MissingFixedView);
        }
        if self.world.spawn_defaults.is_none() {
            return Err(SceneRunError::MissingSpawnDefaults);
        }
        if self.world.reflect_all_contacts.is_none() {
            return Err(SceneRunError::MissingSecondaryMarkerPolicy);
        }
        // Profiles are captured before deferred retirement.
        let queue = CollisionQueue::build(&self.objects, queue_disabled)
            .map_err(SceneRunError::Collision)?;
        let mut host = self.host(catalog);
        host.clean_epoch()?;
        queue
            .detect(host.objects, &mut host.world.contacts, host.world.strategy_clock as u8)
            .map_err(SceneRunError::Collision)?;
        host.advance_player_palette()?;
        frame_background::publish(host.objects, host.world, &mut host.execution.paths.runtime)
            .map_err(SceneRunError::Background)?;
        Ok(())
    }

    /// The strategy epoch (`$7F:34E7`/`$7F:354A`) with its entropy refresh.
    pub fn run_epoch(
        &mut self,
        catalog: &PathCatalog,
        refresh: EntropyRefresh<'_>,
    ) -> Result<(), SceneRunError<C::Error>> {
        let mut schedule = self.schedule;
        let mut host = self.host(catalog);
        host.begin_strategy_epoch(&mut schedule)
            .map_err(SceneRunError::Schedule)?;
        if let EntropyRefresh::BeforeDraws(draws) = refresh {
            if !host.world.random.schedule_refreshes(draws) {
                return Err(SceneRunError::InvalidRefreshSchedule);
            }
        }
        let mut visits = ListenerRefresh { host };
        // No render work is pending natively: the overlapping half yields at
        // once and the remainder visits every actor from the saved cursor.
        schedule
            .run_overlapping(&mut visits, || false)
            .map_err(SceneRunError::Schedule)?;
        schedule
            .run_remainder(&mut visits)
            .map_err(SceneRunError::Schedule)?;
        // A refresh after the last draw (or after the pass) happens now.
        let random = &mut visits.host.world.random;
        match refresh {
            EntropyRefresh::BeforeDraws(_) => random.finish_refreshes(),
            EntropyRefresh::AfterPass => {
                random.next_byte();
            }
        }
        self.schedule = schedule;
        Ok(())
    }

    fn host<'a>(&'a mut self, catalog: &'a PathCatalog) -> SceneActors<'a, C> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog,
            callbacks: &mut self.callbacks,
            statement_budget: STATEMENT_BUDGET,
        }
    }
}

/// Sound listener markers and the positional-loop listener follow the fixed
/// view, which scene actors move during the pass. They are refreshed before
/// each actor visit from the live view pose.
struct ListenerRefresh<'a, C: SceneCallbacks> {
    host: SceneActors<'a, C>,
}

impl<C: SceneCallbacks> ListenerRefresh<'_, C> {
    fn refresh(&mut self) -> Result<(), SceneError<C::Error>> {
        let view = self.host.world.fixed_players[0].expect("validated fixed view");
        let camera = self
            .host
            .objects
            .get(view)
            .ok_or(SceneError::MissingActor(view))?;
        let marker = CueMarker {
            identity: CueListener::PrimaryFallback,
            position: camera.base.position,
            bearing: FixedViewAngles::capture(camera).heading(),
        };
        self.host.world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
            markers: Some(MarkerInputs {
                selected_sides: [PlayerTarget::Primary; 2],
                markers: [marker; 2],
            }),
        });
        self.host.execution.controls.loop_listener = Some(LoopListener {
            position: marker.position,
            bearing: marker.bearing,
        });
        // Death cues use the fixed view markers; scene flag 1AA6 bit 02
        // suppresses the second one.
        let secondary_marker = if self.host.world.reflect_all_contacts.expect("validated policy") {
            None
        } else {
            let second = self.host.world.fixed_players[1].ok_or(SceneError::MissingActor(view))?;
            Some(self.host.objects.get(second).ok_or(SceneError::MissingActor(second))?.base.position)
        };
        self.host.execution.controls.death_effects = Some(super::common_destruction::EffectInputs {
            spawn: self.host.world.spawn_defaults.expect("validated spawn defaults"),
            primary_marker: marker.position,
            secondary_marker,
        });
        Ok(())
    }
}

impl<C: SceneCallbacks> StrategyHost for ListenerRefresh<'_, C> {
    type Error = SceneError<C::Error>;
    fn objects(&self) -> &ObjectStore {
        self.host.objects
    }
    fn strategy_suspended(&self, object: ObjectId) -> bool {
        self.host.strategy_suspended(object)
    }
    fn run_strategy(
        &mut self,
        object: ObjectId,
        strategy_clock: u16,
    ) -> Result<StrategyCompletion, Self::Error> {
        self.refresh()?;
        self.host.run_strategy(object, strategy_clock)
    }
    fn retire_object(&mut self, object: ObjectId) -> Result<(), Self::Error> {
        self.host.retire_object(object)
    }
}
