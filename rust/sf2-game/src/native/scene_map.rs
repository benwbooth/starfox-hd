//! Scene-map control ownership, independent of source address encodings.
//!
//! Catalog lowering supplies typed scene effects and actor constructors. The
//! caller supplies their real services; this module neither interprets source
//! instructions nor treats an unavailable loader/constructor as successful.
//! Yield markers are retained values, not countdowns. The frame owner invokes
//! the map once at its source-defined point in each eligible update.

use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId, Vector3,
};

const PHASE_HOLD_MARKER: u16 = 5_000;

/// Dense index in one decoded scene-map catalog, not a source stream offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MapCursor(u16);

impl MapCursor {
    pub const fn from_index(index: u16) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapCondition {
    ModeEquals(u8),
    ModeLowBitSet,
    /// Source map branch A4 tests bit 0400 of the external event word.
    ExternalEvent,
    /// Boot's inline branch tests the shared single-player display policy.
    SinglePlayer,
    /// Encounter inline branches test bit 2000 of the shared scene events.
    SceneEvent,
    /// Both the fade progress and the published display state must be ready.
    DisplayReady,
    /// Read the selected load-table entry, not the pending load request.
    LoadTableIdle,
    /// Map branch 9E: the encounter layout (1BA5) equals the operand.
    EncounterLayout(u8),
    /// Map branch A2: encounter layout (1BA5) bit 01.
    EncounterLayoutOdd,
}

/// Effects and spawn specifications are typed catalog data provided by the
/// scene. In particular, neither parameter is an encoded variable operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapInstruction<Effect, Spawn> {
    Stop,
    Yield {
        marker: u16,
        next: MapCursor,
    },
    Jump(MapCursor),
    Branch {
        condition: MapCondition,
        taken: MapCursor,
        otherwise: MapCursor,
    },
    Await {
        condition: MapCondition,
        /// Display waits publish one; load waits preserve the old marker.
        retry_marker: Option<u16>,
        next: MapCursor,
    },
    Apply {
        effect: Effect,
        next: MapCursor,
    },
    ApplyToCurrent {
        effect: Effect,
        next: MapCursor,
    },
    Spawn {
        specification: Spawn,
        marker: u16,
        next: MapCursor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseExit {
    pub parked: MapCursor,
    pub continuation: MapCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    Empty,
    TooLarge,
    InvalidCursor(MapCursor),
    DuplicatePhaseExit(MapCursor),
    InvalidPhaseExit(MapCursor),
}

/// Validation is separate from execution so a malformed decoded graph cannot
/// perform an effect before discovering a missing successor.
pub struct MapCatalog<'a, Effect, Spawn> {
    instructions: &'a [MapInstruction<Effect, Spawn>],
    phase_exits: &'a [PhaseExit],
}

impl<'a, Effect, Spawn> MapCatalog<'a, Effect, Spawn> {
    pub fn new(
        instructions: &'a [MapInstruction<Effect, Spawn>],
        phase_exits: &'a [PhaseExit],
    ) -> Result<Self, CatalogError> {
        if instructions.is_empty() {
            return Err(CatalogError::Empty);
        }
        if instructions.len() > usize::from(u16::MAX) + 1 {
            return Err(CatalogError::TooLarge);
        }
        let catalog = Self {
            instructions,
            phase_exits,
        };
        for instruction in instructions {
            match instruction {
                MapInstruction::Stop => {}
                MapInstruction::Jump(next)
                | MapInstruction::Yield { next, .. }
                | MapInstruction::Await { next, .. }
                | MapInstruction::Apply { next, .. }
                | MapInstruction::ApplyToCurrent { next, .. }
                | MapInstruction::Spawn { next, .. } => catalog.check(*next)?,
                MapInstruction::Branch {
                    taken, otherwise, ..
                } => {
                    catalog.check(*taken)?;
                    catalog.check(*otherwise)?;
                }
            }
        }
        for (index, exit) in phase_exits.iter().enumerate() {
            catalog.check(exit.parked)?;
            catalog.check(exit.continuation)?;
            if phase_exits[..index]
                .iter()
                .any(|prior| prior.parked == exit.parked)
            {
                return Err(CatalogError::DuplicatePhaseExit(exit.parked));
            }
            let MapInstruction::Jump(hold) = &instructions[exit.parked.index()] else {
                return Err(CatalogError::InvalidPhaseExit(exit.parked));
            };
            if !matches!(
                &instructions[hold.index()],
                MapInstruction::Yield { marker: PHASE_HOLD_MARKER, next } if *next == exit.parked
            ) {
                return Err(CatalogError::InvalidPhaseExit(exit.parked));
            }
        }
        Ok(catalog)
    }

    fn check(&self, cursor: MapCursor) -> Result<(), CatalogError> {
        if cursor.index() < self.instructions.len() {
            Ok(())
        } else {
            Err(CatalogError::InvalidCursor(cursor))
        }
    }
}

/// Scene services share the same actor pool and state used by strategy visits.
/// A failed spawn is an ordinary null result; a failed service is a fault.
/// Constructors must use source list insertion/defaults, and current-object
/// effects must retain source write widths. There are no default successes.
pub trait SceneMapHost<Effect, Spawn> {
    type Error;

    fn condition(&self, condition: MapCondition) -> Result<bool, Self::Error>;
    fn apply(&mut self, effect: &Effect) -> Result<(), Self::Error>;
    fn apply_to_current(&mut self, actor: ObjectId, effect: &Effect) -> Result<(), Self::Error>;
    fn spawn(&mut self, specification: &Spawn) -> Result<Option<ObjectId>, Self::Error>;
}

/// Decoded ordinary map-spawn record. Scene-specific strategy state is bound
/// by the owning host after allocation; it must not run its first visit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapActorSpawn {
    pub kind: ObjectKind,
    pub shape: ShapeId,
    pub behavior: Behavior,
    pub position: Vector3,
}

/// Map allocation inserts after the current list head, not the map's previous
/// selected object. This is the source ordinary-spawn handler ($03:9EC8): no
/// pressure sweep, no child attachment, and no immediate strategy execution.
pub fn allocate_map_actor(
    objects: &mut ObjectStore,
    defaults: ObjectSpawnDefaults,
    spawn: MapActorSpawn,
) -> Option<ObjectId> {
    let after = objects.active_ids().first().copied();
    let mut actor = Object::new_authored(spawn.kind, spawn.shape, spawn.behavior, defaults);
    actor.base.position = spawn.position;
    objects.allocate_without_pressure_after(after, actor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapStop {
    Stopped,
    Yielded(u16),
    Waiting(MapCondition),
    Suppressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapReport {
    pub commands: usize,
    pub stop: MapStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError<E> {
    Catalog(CatalogError),
    Host(E),
    BudgetExhausted,
    Faulted,
}

/// Only their conjunction suppresses the map. The ordinary frame path still
/// dispatches with the map-suppression bit alone ($03:8069/$03:8125).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapFramePolicy {
    pub alternate_view: bool,
    pub suppress_alternate_map: bool,
}

impl MapFramePolicy {
    pub const fn dispatches(self) -> bool {
        !self.alternate_view || !self.suppress_alternate_map
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneMap {
    cursor: MapCursor,
    saved_continuation: Option<MapCursor>,
    marker: u16,
    current: Option<ObjectId>,
    faulted: bool,
}

impl SceneMap {
    pub fn new<Effect, Spawn>(
        catalog: &MapCatalog<'_, Effect, Spawn>,
        entry: MapCursor,
    ) -> Result<Self, CatalogError> {
        catalog.check(entry)?;
        Ok(Self {
            cursor: entry,
            saved_continuation: None,
            marker: 0,
            current: None,
            faulted: false,
        })
    }

    pub fn cursor(&self) -> MapCursor {
        self.cursor
    }
    pub fn yield_marker(&self) -> u16 {
        self.marker
    }
    pub fn current_object(&self) -> Option<ObjectId> {
        self.current
    }
    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    /// The authored map publishes this continuation before entering a
    /// mission. Actor-side restoration changes only the live cursor: marker,
    /// current actor and the saved continuation itself all survive.
    pub fn save_continuation<Effect, Spawn>(
        &mut self,
        catalog: &MapCatalog<'_, Effect, Spawn>,
        target: MapCursor,
    ) -> Result<(), MapError<std::convert::Infallible>> {
        if self.faulted {
            return Err(MapError::Faulted);
        }
        catalog.check(target).map_err(MapError::Catalog)?;
        self.saved_continuation = Some(target);
        Ok(())
    }

    pub fn saved_continuation(&self) -> Option<MapCursor> {
        self.saved_continuation
    }

    /// $7F:BF3D. The continuation is a validated native catalog identity,
    /// never the source bank/address pair used to encode that identity.
    pub fn restore_continuation(&mut self) -> Result<(), MapRestoreError> {
        if self.faulted {
            return Err(MapRestoreError::Faulted);
        }
        self.cursor = self
            .saved_continuation
            .ok_or(MapRestoreError::MissingContinuation)?;
        Ok(())
    }

    /// An outer scene transition replaces only the cursor. It does not erase
    /// the last selected actor or a marker published by a preceding command.
    /// Fault recovery must construct a new owner, not replay partial effects.
    pub fn redirect<Effect, Spawn>(
        &mut self,
        catalog: &MapCatalog<'_, Effect, Spawn>,
        target: MapCursor,
    ) -> Result<(), MapError<std::convert::Infallible>> {
        if self.faulted {
            return Err(MapError::Faulted);
        }
        catalog.check(target).map_err(MapError::Catalog)?;
        self.cursor = target;
        Ok(())
    }

    /// Apply an explicitly requested, catalog-proven outer phase transition.
    /// No timer or controller input is invented by this operation.
    pub fn release_phase<Effect, Spawn>(
        &mut self,
        catalog: &MapCatalog<'_, Effect, Spawn>,
    ) -> Result<bool, MapError<std::convert::Infallible>> {
        if self.faulted {
            return Err(MapError::Faulted);
        }
        if let Some(exit) = catalog
            .phase_exits
            .iter()
            .find(|exit| exit.parked == self.cursor)
        {
            self.cursor = exit.continuation;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn visit<Effect, Spawn, Host: SceneMapHost<Effect, Spawn>>(
        &mut self,
        catalog: &MapCatalog<'_, Effect, Spawn>,
        host: &mut Host,
        policy: MapFramePolicy,
        budget: usize,
    ) -> Result<MapReport, MapError<Host::Error>> {
        if self.faulted {
            return Err(MapError::Faulted);
        }
        if !policy.dispatches() {
            return Ok(MapReport {
                commands: 0,
                stop: MapStop::Suppressed,
            });
        }
        let result = self.dispatch(catalog, host, budget);
        if result.is_err() {
            self.faulted = true;
        }
        result
    }

    fn dispatch<Effect, Spawn, Host: SceneMapHost<Effect, Spawn>>(
        &mut self,
        catalog: &MapCatalog<'_, Effect, Spawn>,
        host: &mut Host,
        budget: usize,
    ) -> Result<MapReport, MapError<Host::Error>> {
        for commands in 1..=budget {
            catalog.check(self.cursor).map_err(MapError::Catalog)?;
            let stop = match &catalog.instructions[self.cursor.index()] {
                MapInstruction::Stop => Some(MapStop::Stopped),
                MapInstruction::Yield { marker, next } => {
                    self.cursor = *next;
                    if *marker == 0 {
                        None
                    } else {
                        self.marker = *marker;
                        Some(MapStop::Yielded(*marker))
                    }
                }
                MapInstruction::Jump(target) => {
                    self.cursor = *target;
                    None
                }
                MapInstruction::Branch {
                    condition,
                    taken,
                    otherwise,
                } => {
                    self.cursor = if host.condition(*condition).map_err(MapError::Host)? {
                        *taken
                    } else {
                        *otherwise
                    };
                    None
                }
                MapInstruction::Await {
                    condition,
                    retry_marker,
                    next,
                } => {
                    if host.condition(*condition).map_err(MapError::Host)? {
                        self.cursor = *next;
                        None
                    } else {
                        if let Some(marker) = retry_marker {
                            self.marker = *marker;
                        }
                        Some(MapStop::Waiting(*condition))
                    }
                }
                MapInstruction::Apply { effect, next } => {
                    host.apply(effect).map_err(MapError::Host)?;
                    self.cursor = *next;
                    None
                }
                MapInstruction::ApplyToCurrent { effect, next } => {
                    if let Some(actor) = self.current {
                        host.apply_to_current(actor, effect)
                            .map_err(MapError::Host)?;
                    }
                    self.cursor = *next;
                    None
                }
                MapInstruction::Spawn {
                    specification,
                    marker,
                    next,
                } => {
                    // Unlike a zero delay, a zero spawn operand DOES publish
                    // zero before allocation ($03:9EC9), even on pool failure.
                    self.marker = *marker;
                    self.current = host.spawn(specification).map_err(MapError::Host)?;
                    self.cursor = *next;
                    (*marker != 0).then_some(MapStop::Yielded(*marker))
                }
            };
            if let Some(stop) = stop {
                return Ok(MapReport { commands, stop });
            }
        }
        Err(MapError::BudgetExhausted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapRestoreError {
    MissingContinuation,
    Faulted,
}

#[cfg(test)]
#[path = "scene_map_tests.rs"]
mod tests;
