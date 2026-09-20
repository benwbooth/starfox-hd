//! Actor auxiliary lookup/insert (`$7F:233B..23D1`). Keys and payloads are
//! decoded domain variants, not numeric source types or packed storage.
//! All records share the actor's one counted table and program-resource pool.

use super::program_resources::{
    AllocationFailure, ProgramResourceId, ProgramResources, ReplacementError,
};
use super::program_state::ProgramData;
use super::{ObjectId, PathCursor};

const COUNT_COST: u16 = 1;
const ENTRY_COST: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryKind {
    SceneContinuation,
    SavedView,
    OrdinaryImpactMaterial,
    SuppressedImpactMaterial,
    ReflectionShape,
}

/// Reviewed source types 3, 8, 10, 11 and 13. Extend with typed payloads as the other
/// auxiliary producers migrate; unknown source kinds are not generic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryRecord {
    SceneContinuation(PathCursor),
    SavedView(ProgramResourceId),
    OrdinaryImpactMaterial(u8),
    SuppressedImpactMaterial(u8),
    ReflectionShape(super::ShapeId),
}

impl AuxiliaryRecord {
    pub const fn kind(self) -> AuxiliaryKind {
        match self {
            Self::SceneContinuation(_) => AuxiliaryKind::SceneContinuation,
            Self::SavedView(_) => AuxiliaryKind::SavedView,
            Self::OrdinaryImpactMaterial(_) => AuxiliaryKind::OrdinaryImpactMaterial,
            Self::SuppressedImpactMaterial(_) => AuxiliaryKind::SuppressedImpactMaterial,
            Self::ReflectionShape(_) => AuxiliaryKind::ReflectionShape,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AuxiliaryRecords {
    entries: Vec<AuxiliaryRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryError {
    MissingStorage,
    CountOverflow,
    StorageStillOwned,
    Allocation(AllocationFailure),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ActorAuxiliary {
    storage: Option<ProgramResourceId>,
}

impl ActorAuxiliary {
    pub fn reflection_shape(
        &self,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<Option<super::ShapeId>, AuxiliaryError> {
        Ok(
            match self.find(resources, owner, AuxiliaryKind::ReflectionShape)? {
                Some(AuxiliaryRecord::ReflectionShape(shape)) => Some(shape),
                _ => None,
            },
        )
    }

    pub fn impact_materials(
        &self,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<super::path_impact::ImpactMaterials, AuxiliaryError> {
        let ordinary = match self.find(resources, owner, AuxiliaryKind::OrdinaryImpactMaterial)? {
            Some(AuxiliaryRecord::OrdinaryImpactMaterial(value)) => Some(value),
            _ => None,
        };
        let suppressed =
            match self.find(resources, owner, AuxiliaryKind::SuppressedImpactMaterial)? {
                Some(AuxiliaryRecord::SuppressedImpactMaterial(value)) => Some(value),
                _ => None,
            };
        Ok(super::path_impact::ImpactMaterials {
            ordinary,
            suppressed,
        })
    }

    pub fn scene_continuation(
        &self,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<Option<PathCursor>, AuxiliaryError> {
        Ok(
            match self.find(resources, owner, AuxiliaryKind::SceneContinuation)? {
                Some(AuxiliaryRecord::SceneContinuation(path)) => Some(path),
                _ => None,
            },
        )
    }

    pub fn entries<'a>(
        &self,
        resources: &'a ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<&'a [AuxiliaryRecord], AuxiliaryError> {
        let Some(id) = self.storage else {
            return Ok(&[]);
        };
        match resources.get_owned(owner, id) {
            Some(ProgramData::ActorAuxiliary(records)) => Ok(&records.entries),
            _ => Err(AuxiliaryError::MissingStorage),
        }
    }

    pub fn find(
        &self,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
        kind: AuxiliaryKind,
    ) -> Result<Option<AuxiliaryRecord>, AuxiliaryError> {
        Ok(self
            .entries(resources, owner)?
            .iter()
            .copied()
            .find(|entry| entry.kind() == kind))
    }

    /// Updating a present kind does not resize or allocate. Every new kind
    /// grows the table to exactly one plus four times the count, allocating
    /// the replacement before freeing the old table. Payload ownership is
    /// deliberately separate: replacing a reference never frees its target.
    pub fn set(
        &mut self,
        resources: &mut ProgramResources<ProgramData>,
        owner: ObjectId,
        record: AuxiliaryRecord,
    ) -> Result<(), AuxiliaryError> {
        let entries = self.entries(resources, owner)?;
        if let Some(index) = entries
            .iter()
            .position(|entry| entry.kind() == record.kind())
        {
            let Some(ProgramData::ActorAuxiliary(table)) =
                resources.get_owned_mut(owner, self.storage.expect("nonempty auxiliary table"))
            else {
                return Err(AuxiliaryError::MissingStorage);
            };
            table.entries[index] = record;
            return Ok(());
        }
        if entries.len() == usize::from(u8::MAX) {
            // The source wraps the byte count to zero and then copies into
            // an undersized allocation; that is not a valid typed table.
            return Err(AuxiliaryError::CountOverflow);
        }
        let mut entries = entries.to_vec();
        entries.push(record);
        let cost = COUNT_COST + ENTRY_COST * entries.len() as u16;
        let data = ProgramData::ActorAuxiliary(AuxiliaryRecords { entries });
        self.storage = Some(match self.storage {
            None => resources
                .allocate_owned(owner, cost, data)
                .map_err(|error| AuxiliaryError::Allocation(error.reason))?,
            Some(old) => resources
                .replace_owned(owner, old, cost, data)
                .map_err(|error| match error {
                    ReplacementError::MissingOwnedResource(_) => AuxiliaryError::MissingStorage,
                    ReplacementError::Allocation(error) => AuxiliaryError::Allocation(error.reason),
                })?,
        });
        Ok(())
    }

    /// Actor retirement releases the entire ownership chain first. Merely
    /// forgetting this table must never hide still-owned allocations.
    pub fn clear_after_owner_release(
        &mut self,
        resources: &ProgramResources<ProgramData>,
        owner: ObjectId,
    ) -> Result<(), AuxiliaryError> {
        if let Some(id) = self.storage {
            if resources.get_owned(owner, id).is_some() {
                return Err(AuxiliaryError::StorageStillOwned);
            }
            if resources.get(id).is_some() {
                return Err(AuxiliaryError::MissingStorage);
            }
        }
        self.storage = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "actor_auxiliary_tests.rs"]
mod tests;
