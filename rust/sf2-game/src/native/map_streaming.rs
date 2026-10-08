//! Map regions and proximity-streamed map records.
//!
//! * Region tracking (`$0D:D95B..DA5E`): the primary player's position selects
//!   up to two registered rectangles. The current and previous region become
//!   the allocation groups (190E/190F); leaving both retires a group.
//! * Group retirement (`$0D:D8DD..D95A`): every actor and map record of the
//!   retired group is released.
//! * Streaming (`$0D:D5FA..D8DC`): map records spawn an actor while a view
//!   is within their radius and mark it for removal when it leaves.
//!
//! Records are produced by the map loader (opcode `$90`) and regions by
//! opcode `$94`; this module only owns their runtime behavior. Record and
//! actor lists keep the source "insert after head" order, which fixes the
//! order of later spawns and therefore of strategy visits.

use super::scene_map::MapCursor;
use super::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, PathCursor,
    ShapeId, Vector3,
};

pub const MAP_RECORD_CAPACITY: usize = 512;
pub const REGION_CAPACITY: usize = 128;
/// Region id stored in 190E/190F when no region is selected.
pub const NO_REGION: u8 = 0xFF;
/// Spawned records keep their actor until it is this much farther out.
const SPAWNED_RADIUS_MARGIN: u16 = 500;
const SHAPE_EXTENT_SHIFT: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapRecordId(u16);

impl MapRecordId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A registered map region (`$686A + 16 * id`, written by map opcode 94).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapRegion {
    pub origin_x: u16,
    pub origin_z: u16,
    pub width: u16,
    pub depth: u16,
    /// The map script (0C/0E, published to 1657/192E) entered with the
    /// region. Only scannable regions (byte 0F bit 1) have one; the scan
    /// ignores the others.
    pub entry: Option<MapCursor>,
}

impl MapRegion {
    fn contains(&self, x: u16, z: u16) -> bool {
        self.entry.is_some()
            && x.wrapping_sub(self.origin_x) < self.width
            && z.wrapping_sub(self.origin_z) < self.depth
    }
}

/// Published when the player enters a region it was not already in: the
/// map owner redirects the scene map to the region's script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionPublication {
    pub entry: MapCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapProgram {
    /// Record flag bit 1: follow this authored path (strategy `$7F:7E1E`).
    Path(PathCursor),
    /// Any other strategy, already resolved by the record's producer.
    Strategy(Behavior),
}

/// Record byte 12.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapRecordFlags(pub u8);

impl MapRecordFlags {
    pub const SPAWNED: u8 = 0x01;
    pub const PATH: u8 = 0x02;
    pub const ATTACHED_DATA: u8 = 0x04;
    pub const FIXED_RADIUS: u8 = 0x08;
    pub const SUPPRESSED: u8 = 0x10;

    pub const fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapRecord {
    pub position: Vector3,
    pub rotation: [Angle; 3],
    pub shape: ShapeId,
    /// Word 10 of the record's shape header; drives the spawn radius.
    pub shape_extent: u16,
    pub kind: ObjectKind,
    pub program: MapProgram,
    pub flags: MapRecordFlags,
    /// Actor last spawned from this record. The source leaves this link in
    /// place when it releases the actor; only the spawned flag is cleared.
    pub spawned: Option<ObjectId>,
    /// Allocation group (byte 18).
    pub group: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRecordStore {
    slots: Vec<Option<MapRecord>>,
    active: Vec<MapRecordId>,
    free: Vec<MapRecordId>,
}

impl Default for MapRecordStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MapRecordStore {
    /// `$0D:D5C1..D5D8`: an empty list and a free list in slot order.
    pub fn new() -> Self {
        Self {
            slots: vec![None; MAP_RECORD_CAPACITY],
            active: Vec::new(),
            free: (0..MAP_RECORD_CAPACITY as u16)
                .rev()
                .map(MapRecordId)
                .collect(),
        }
    }

    pub fn active_ids(&self) -> &[MapRecordId] {
        &self.active
    }

    pub fn get(&self, id: MapRecordId) -> Option<&MapRecord> {
        self.slots.get(id.index())?.as_ref()
    }

    pub fn get_mut(&mut self, id: MapRecordId) -> Option<&mut MapRecord> {
        self.slots.get_mut(id.index())?.as_mut()
    }

    /// `$7F:2ADF`: pop the free list and insert after the head (or become
    /// the head of an empty list). `None` when the pool is exhausted.
    pub fn allocate(&mut self, record: MapRecord) -> Option<MapRecordId> {
        let id = self.free.pop()?;
        self.slots[id.index()] = Some(record);
        let position = usize::from(!self.active.is_empty());
        self.active.insert(position, id);
        Some(id)
    }

    fn release(&mut self, id: MapRecordId) {
        if let Some(position) = self.active.iter().position(|candidate| *candidate == id) {
            self.active.remove(position);
            self.slots[id.index()] = None;
            self.free.push(id);
        }
    }
}

/// The region table and its count (1910). Map opcode 94 writes the slot it
/// names and counts one more region; the scan covers the first `count`
/// slots, so a slot below the count that was never written is a fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRegions {
    slots: Vec<Option<MapRegion>>,
    count: usize,
}

impl Default for MapRegions {
    fn default() -> Self {
        Self {
            slots: vec![None; REGION_CAPACITY],
            count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionError {
    /// The source table holds 128 records; a further count reads past it.
    Overflow,
    /// The scan would read a slot no map record has written.
    Unwritten(u8),
}

impl MapRegions {
    pub fn new(regions: Vec<MapRegion>) -> Self {
        assert!(regions.len() <= REGION_CAPACITY, "region table overflow");
        let mut table = Self::default();
        for (index, region) in regions.into_iter().enumerate() {
            table.slots[index] = Some(region);
        }
        table.count = table.slots.iter().take_while(|slot| slot.is_some()).count();
        table
    }

    /// `$03:90EF`: write slot `index` and count one more region.
    pub fn register(&mut self, index: u8, region: MapRegion) -> Result<(), RegionError> {
        let slot = self
            .slots
            .get_mut(usize::from(index))
            .ok_or(RegionError::Overflow)?;
        if self.count == REGION_CAPACITY {
            return Err(RegionError::Overflow);
        }
        *slot = Some(region);
        self.count += 1;
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// The scanned regions, in slot order.
    pub fn regions(&self) -> Result<Vec<MapRegion>, RegionError> {
        self.slots[..self.count]
            .iter()
            .enumerate()
            .map(|(index, slot)| slot.ok_or(RegionError::Unwritten(index as u8)))
            .collect()
    }
}

/// Region selection carried between frames: 190E (current) and 190F.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionGroups {
    pub current: u8,
    pub previous: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionUpdate {
    pub groups: RegionGroups,
    pub publication: Option<RegionPublication>,
    /// Groups left this frame, in source retirement order.
    pub retired: [Option<u8>; 2],
}

/// `$0D:D95B..DA5E` without its retirement calls, which the caller performs
/// in the returned order (see [`retire_group`]).
pub fn select_regions(
    regions: &MapRegions,
    player: Vector3,
    old: RegionGroups,
) -> Result<RegionUpdate, RegionError> {
    let (x, z) = (player.x as u16, player.z as u16);
    let mut current = NO_REGION;
    let mut previous = NO_REGION;
    let mut publication = None;
    for (index, region) in regions.regions()?.iter().enumerate() {
        let id = index as u8;
        if !region.contains(x, z) {
            continue;
        }
        if id != old.current && id != old.previous {
            publication = region.entry.map(|entry| RegionPublication { entry });
        }
        // The previous current region becomes the second one, unless none.
        if current != NO_REGION {
            previous = current;
        }
        current = id;
        if (id == old.previous || id == old.current)
            && previous != NO_REGION
            && previous != old.previous
        {
            current = previous;
            previous = id;
        }
    }
    if current == previous {
        // Only possible when no region matched: keep the old selection.
        return Ok(RegionUpdate {
            groups: old,
            publication,
            retired: [None; 2],
        });
    }
    let retire =
        |group: u8| (group != NO_REGION && group != current && group != previous).then_some(group);
    Ok(RegionUpdate {
        groups: RegionGroups { current, previous },
        publication,
        retired: [retire(old.current), retire(old.previous)],
    })
}

/// `$0D:D8DD..D95A`: mark every live actor of `group` for removal and clear
/// its record link, then release every map record of `group`.
pub fn retire_group(objects: &mut ObjectStore, records: &mut MapRecordStore, group: u8) {
    for id in objects.active_ids().to_vec() {
        let actor = objects.get_mut(id).expect("live actor list");
        if actor.extension.spawn_group == group {
            actor.extension.map_record = None;
            actor.base.flags.remove_after_tick = true;
        }
    }
    for id in records.active_ids().to_vec() {
        if records.get(id).expect("live record list").group == group {
            records.release(id);
        }
    }
}

/// Streaming state carried between frames.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StreamingState {
    /// CF35: frames of forced rescans left after a region change.
    pub rescan_countdown: u8,
    /// CF21..CF28: view positions of the last scan (only high bytes compared).
    pub scanned_views: [(u16, u16); 2],
}

/// Inputs sampled by `$0D:D5FA` from shared flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamingInputs {
    /// 1E16 bit 80: skip the region update this frame.
    pub hold_regions: bool,
    /// 1B84 bit 02: scripted view; no streaming at all.
    pub scripted_view: bool,
    /// 1AA6 bit 01: the second view (037E) also streams records in.
    pub second_view_streams: bool,
    /// 1AA6 bit 02: an unchanged first-view cell ends the scan by itself.
    pub first_view_decides: bool,
    /// D739: radius ceiling.
    pub radius_limit: u16,
    /// 1A83: radius added to every shape extent.
    pub base_radius: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingError {
    /// Records with attached data (flag bit 2) need the program-resource
    /// attachment (`$7F:2360`), which is not ported yet.
    UnsupportedAttachedData(MapRecordId),
    MissingSpawnDefaults,
    Region(RegionError),
}

/// Region update hooks provided by the frame owner.
pub struct StreamingWorld<'a> {
    pub inputs: StreamingInputs,
    pub state: &'a mut StreamingState,
    pub regions: &'a MapRegions,
    pub groups: &'a mut RegionGroups,
    pub publication: &'a mut Option<RegionPublication>,
    pub primary_player: Vector3,
    /// Fixed views 033F and 037E (their world positions).
    pub views: [Vector3; 2],
    /// Fresh-object defaults; the group is overwritten per record.
    pub spawn_defaults: Option<ObjectSpawnDefaults>,
}

fn high(word: u16) -> u8 {
    (word >> 8) as u8
}

/// One `$0D:D5FA` visit.
pub fn stream(
    objects: &mut ObjectStore,
    records: &mut MapRecordStore,
    world: StreamingWorld<'_>,
) -> Result<(), StreamingError> {
    let StreamingWorld {
        inputs,
        state,
        regions,
        groups,
        publication,
        primary_player,
        views,
        spawn_defaults,
    } = world;
    let mut force = false;
    if !inputs.hold_regions {
        if inputs.scripted_view {
            return Ok(());
        }
        let old = *groups;
        let update =
            select_regions(regions, primary_player, old).map_err(StreamingError::Region)?;
        *groups = update.groups;
        if let Some(entry) = update.publication {
            *publication = Some(entry);
        }
        for group in update.retired.into_iter().flatten() {
            retire_group(objects, records, group);
        }
        if groups.current != old.current {
            state.rescan_countdown = 1;
            force = true;
        }
    }
    if !force {
        if state.rescan_countdown != 0 {
            state.rescan_countdown -= 1;
        } else {
            // Both cell tests read the FIRST view's position. The second
            // compares it with the second view's saved cell (`$0D:D65C`
            // reloads 033F, not 037E); this is source behavior, kept as is.
            let first = views[0];
            let same = |index: usize| {
                let (x, z) = state.scanned_views[index];
                high(x) == high(first.x as u16) && high(z) == high(first.z as u16)
            };
            if same(0) && (inputs.first_view_decides || same(1)) {
                return Ok(());
            }
        }
    }
    for (slot, view) in state.scanned_views.iter_mut().zip(views) {
        *slot = (view.x as u16, view.z as u16);
    }
    for id in records.active_ids().to_vec() {
        let record = *records.get(id).expect("live record list");
        let mut inside = in_range(objects, &record, views[0], inputs);
        if !inside && inputs.second_view_streams {
            inside = in_range(objects, &record, views[1], inputs);
        }
        let record = records.get_mut(id).expect("live record list");
        if !inside {
            if record.flags.has(MapRecordFlags::SPAWNED) {
                let actor = record.spawned.expect("spawned record has an actor");
                if let Some(actor) = objects.get_mut(actor) {
                    actor.base.flags.remove_after_tick = true;
                }
                record.flags.0 &= !MapRecordFlags::SPAWNED;
            } else {
                record.flags.0 &= !MapRecordFlags::SUPPRESSED;
            }
            continue;
        }
        if record.flags.has(MapRecordFlags::SPAWNED) || record.flags.has(MapRecordFlags::SUPPRESSED)
        {
            continue;
        }
        if record.flags.has(MapRecordFlags::ATTACHED_DATA) {
            return Err(StreamingError::UnsupportedAttachedData(id));
        }
        let defaults = spawn_defaults.ok_or(StreamingError::MissingSpawnDefaults)?;
        let behavior = match record.program {
            MapProgram::Path(_) => Behavior::FollowPath,
            MapProgram::Strategy(behavior) => behavior,
        };
        let mut actor = Object::new_authored(record.kind, record.shape, behavior, defaults);
        actor.base.position = record.position;
        if let MapProgram::Path(path) = record.program {
            actor.base.path = Some(path);
            actor.extension.path_state.needs_path_initialization = true;
        }
        actor.extension.map_record = Some(id);
        actor.base.pitch = record.rotation[0];
        actor.base.yaw = record.rotation[1];
        actor.base.roll = record.rotation[2];
        actor.extension.spawn_group = record.group;
        // The map allocator has no last-slot pressure sweep; a full pool
        // skips this record until a later scan.
        let head = objects.active_ids().first().copied();
        let Some(spawned) = objects.allocate_without_pressure_after(head, actor) else {
            continue;
        };
        record.flags.0 |= MapRecordFlags::SPAWNED;
        record.spawned = Some(spawned);
    }
    Ok(())
}

fn in_range(
    objects: &ObjectStore,
    record: &MapRecord,
    view: Vector3,
    inputs: StreamingInputs,
) -> bool {
    let mut radius = if record.flags.has(MapRecordFlags::FIXED_RADIUS) {
        inputs.radius_limit
    } else {
        let radius = (record.shape_extent << SHAPE_EXTENT_SHIFT).wrapping_add(inputs.base_radius);
        if radius < inputs.radius_limit {
            radius
        } else {
            inputs.radius_limit
        }
    };
    let position = if record.flags.has(MapRecordFlags::SPAWNED) {
        radius = radius.wrapping_add(SPAWNED_RADIUS_MARGIN);
        let actor = record.spawned.expect("spawned record has an actor");
        // The freed record keeps its last position until slot reuse.
        objects
            .attachment_parent_pose(actor)
            .map(|(position, _)| position)
            .expect("spawned record names a live or freed actor slot")
    } else {
        record.position
    };
    let span = radius.wrapping_add(radius);
    let near = |delta: u16| delta.wrapping_add(radius) < span;
    near((position.x as u16).wrapping_sub(view.x as u16))
        && near((position.z as u16).wrapping_sub(view.z as u16))
}

#[cfg(test)]
#[path = "map_streaming_tests.rs"]
mod tests;
