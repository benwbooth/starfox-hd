//! Complete world-cell response ($07:E685..EA14). This uses the live occupancy
//! plane, reserved view and proxy, and shares heading/contact/history owners
//! with flight. The source's asymmetric diagonal tie is deliberately retained.

use super::path_fields::chase_word;
use super::player_pose::quarter_byte;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::world_occupancy::CELL_SIZE;
use super::{ObjectId, ObjectStore, Vector3};

const VIEW_BLOCKED: u8 = 0x80;
const PLAYER_BLOCKED: u8 = 0x40;
const NEW_OBSTRUCTION: u8 = 0x10;
const MODE_FAMILY: u8 = 0xF0;
const FLIGHT_MODE: u8 = 0x10;
const CELL_SHIFT: u32 = CELL_SIZE.trailing_zeros();
const HALF_CELL: u16 = CELL_SIZE / 2;
const CELL_EDGE: u16 = CELL_SIZE - 1;
const CELL_DELTA_MASK: u8 = 3;
const CELL_DELTA_STRIDE_BITS: u32 = 2;
const DIRECTION_TABLE: [u8; 16] = [224, 0, 32, 0, 192, 255, 64, 0, 160, 128, 96, 0, 0, 0, 0, 0];
const RETURN_HEADING_TABLE: [u8; 16] = [
    192, 64, 224, 96, 0, 128, 32, 160, 64, 192, 96, 224, 128, 0, 160, 32,
];
const EDGE_TABLE: [u8; 8] = [4, 6, 2, 10, 8, 9, 1, 5];
const LOWER_X: u8 = 0x01;
const UPPER_X: u8 = 0x02;
const LOWER_Z: u8 = 0x04;
const UPPER_Z: u8 = 0x08;
const RETURN_BANK_SHIFT: u32 = 3;
const DIRECTION_HALF_OCTANT_SHIFT: u32 = 4;
const DIRECTION_OCTANT_SHIFT: u32 = 5;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerOccupancy {
    /// 6B29/2A: last admitted horizontal cells, as signed byte coordinates.
    pub current_cell: [i8; 2],
    /// 6B2C/2D: prior cells; only an axis that changes replaces its history.
    pub previous_cell: [i8; 2],
    /// 6AF9/FB: retained horizontal correction outside ordinary flight.
    pub displacement: [i16; 2],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OccupancyContext {
    /// Whether the enclosing player's actual camera task is installed
    /// (6A9D). This is distinct from the scene's scripted-view mode.
    pub camera_override_active: Option<bool>,
    /// Upstream diagonal arbitration bias. A broadly admitted surface
    /// candidate clears it, including when the final surface hit is absent.
    /// An empty query preserves the caller's value. Only the two-open-neighbor
    /// branch needs it; an unknown value must not be silently made neutral.
    pub diagonal_tie_bias: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OccupancyError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingMotion(ObjectId),
    MissingExemption(ObjectId),
    MissingCameraTask,
    MissingView,
    MissingOccupancy,
    MissingPlayerOccupancy(ObjectId),
    MissingProxy,
    MissingDiagonalTieBias,
    MissingPose(ObjectId),
    MissingYawMotion(ObjectId),
    MissingYawIncrement,
    MissingBoundary(ObjectId),
}
impl From<WorldInputError> for OccupancyError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}
impl From<PlayerStorageError> for OccupancyError {
    fn from(value: PlayerStorageError) -> Self {
        Self::Storage(value)
    }
}

fn cell(position: Vector3) -> [i8; 2] {
    [
        (position.x >> CELL_SHIFT) as i8,
        (position.z >> CELL_SHIFT) as i8,
    ]
}
fn lower_edge(cell: i8) -> i16 {
    ((cell as u8 as u16) << CELL_SHIFT) as i16
}
fn center(cell: i8) -> i16 {
    (lower_edge(cell) as u16 | HALF_CELL) as i16
}
fn occupied(world: &ScenePathWorld, position: Vector3) -> Result<bool, OccupancyError> {
    Ok(world
        .occupancy
        .as_ref()
        .ok_or(OccupancyError::MissingOccupancy)?
        .contains(position))
}

/// The source publishes both trial cells on its actual shared proxy. Y and
/// every other proxy field survive even when both neighbors are blocked.
fn neighbor_blocked(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    cells: [i8; 2],
) -> Result<bool, OccupancyError> {
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(OccupancyError::MissingProxy)?;
    let actor = objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?;
    actor.base.position.x = center(cells[0]);
    actor.base.position.z = center(cells[1]);
    occupied(world, actor.base.position)
}

fn admitted_cell(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    current: [i8; 2],
    new: [i8; 2],
    context: OccupancyContext,
) -> Result<[i8; 2], OccupancyError> {
    if current[0] == new[0] || current[1] == new[1] {
        return Ok(current);
    }
    let x_open = !neighbor_blocked(objects, world, [new[0], current[1]])?;
    let z_open = !neighbor_blocked(objects, world, [current[0], new[1]])?;
    let choose_x = match (x_open, z_open) {
        (false, false) => return Ok(current),
        (true, false) => true,
        (false, true) => false,
        (true, true) => {
            let bias = context
                .diagonal_tie_bias
                .ok_or(OccupancyError::MissingDiagonalTieBias)?;
            // $07:E7D3 computes but never consumes the X distance. Its Z
            // comparison uses absolute cell center (not player distance)
            // against the new Z byte with the upstream high byte retained.
            let threshold = u16::from_le_bytes([new[1] as u8, bias]);
            center(new[1]).wrapping_abs() as u16 >= threshold
        }
    };
    Ok(if choose_x {
        [new[0], current[1]]
    } else {
        [current[0], new[1]]
    })
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    context: OccupancyContext,
) -> Result<(), OccupancyError> {
    if world
        .action_gate
        .ok_or(WorldInputError::MissingActionGate)?
        .code
        != 0
    {
        return Ok(());
    }
    world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .ok_or(OccupancyError::MissingMotion(owner))?
        .contact_flags &= !VIEW_BLOCKED;
    if world
        .player(objects, owner)?
        .occupancy_exempt
        .ok_or(OccupancyError::MissingExemption(owner))?
    {
        return Ok(());
    }
    if context
        .camera_override_active
        .ok_or(OccupancyError::MissingCameraTask)?
    {
        return Ok(());
    }
    let view = world.fixed_players[0].ok_or(OccupancyError::MissingView)?;
    let view_position = objects
        .get(view)
        .ok_or(WorldInputError::MissingActor(view))?
        .base
        .position;
    if occupied(world, view_position)? {
        world
            .player_mut(objects, owner)?
            .motion
            .as_mut()
            .expect("validated motion")
            .contact_flags |= VIEW_BLOCKED;
    }
    let position = objects.get(owner).expect("validated player").base.position;
    let new = cell(position);
    if !occupied(world, position)? {
        let record = world.player_mut(objects, owner)?;
        let state = record
            .occupancy
            .as_mut()
            .ok_or(OccupancyError::MissingPlayerOccupancy(owner))?;
        for axis in 0..2 {
            if state.current_cell[axis] != new[axis] {
                state.previous_cell[axis] = state.current_cell[axis];
            }
        }
        state.current_cell = new;
        record
            .motion
            .as_mut()
            .expect("validated motion")
            .contact_flags &= !PLAYER_BLOCKED;
        return Ok(());
    }
    let record = world.player_mut(objects, owner)?;
    let motion = record.motion.as_mut().expect("validated motion");
    if motion.contact_flags & PLAYER_BLOCKED == 0 {
        motion.contact_flags |= NEW_OBSTRUCTION;
    }
    motion.contact_flags |= PLAYER_BLOCKED;
    let current = record
        .occupancy
        .ok_or(OccupancyError::MissingPlayerOccupancy(owner))?
        .current_cell;
    let current = admitted_cell(objects, world, current, new, context)?;
    world
        .player_mut(objects, owner)?
        .occupancy
        .as_mut()
        .expect("validated occupancy")
        .current_cell = current;
    let delta_index = (new[0] as u8)
        .wrapping_sub(current[0] as u8)
        .wrapping_add(1)
        & CELL_DELTA_MASK
        | (((new[1] as u8)
            .wrapping_sub(current[1] as u8)
            .wrapping_add(1)
            & CELL_DELTA_MASK)
            << CELL_DELTA_STRIDE_BITS);
    let direction = DIRECTION_TABLE[usize::from(delta_index)];
    let fine_yaw = player_storage::get(objects, resources, owner)?.fine_yaw;
    let pose = world
        .player(objects, owner)?
        .pose
        .ok_or(OccupancyError::MissingPose(owner))?;
    let difference = direction
        .wrapping_sub((fine_yaw >> u8::BITS) as u8)
        .wrapping_sub((pose.turning_lean >> u8::BITS) as u8);
    let heading_index = (direction >> DIRECTION_HALF_OCTANT_SHIFT) | (difference >> (u8::BITS - 1));
    let target = u16::from(RETURN_HEADING_TABLE[usize::from(heading_index)]) << u8::BITS;
    let mode = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode;
    if mode & MODE_FAMILY == FLIGHT_MODE
        && world
            .player(objects, owner)?
            .yaw_motion
            .ok_or(OccupancyError::MissingYawMotion(owner))?
            >> u8::BITS
            == 0
    {
        *world
            .player_yaw_increment
            .as_mut()
            .ok_or(OccupancyError::MissingYawIncrement)? &= 0xFF00;
        let updated = chase_word(fine_yaw, target);
        player_storage::get_mut(objects, resources, owner)?.fine_yaw = updated;
        let bank_target = ((fine_yaw >> u8::BITS) as u8)
            .wrapping_sub((updated >> u8::BITS) as u8)
            .wrapping_shl(RETURN_BANK_SHIFT)
            .wrapping_neg();
        let pose = world
            .player_mut(objects, owner)?
            .pose
            .as_mut()
            .expect("validated pose");
        pose.heading_return_bank = quarter_byte(pose.heading_return_bank as u8, bank_target) as i8;
        let lean_target = (target.wrapping_sub(updated) as i16 / 2) as u16;
        pose.turning_lean = chase_word(pose.turning_lean, lean_target);
    }
    let edges = EDGE_TABLE[usize::from(direction >> DIRECTION_OCTANT_SHIFT)];
    let correction = |cell, position, lower, upper| {
        if edges & lower != 0 {
            lower_edge(cell).wrapping_sub(position)
        } else if edges & upper != 0 {
            (lower_edge(cell) as u16 | CELL_EDGE).wrapping_sub(position as u16) as i16
        } else {
            0
        }
    };
    let delta = [
        correction(current[0], position.x, LOWER_X, UPPER_X),
        correction(current[1], position.z, LOWER_Z, UPPER_Z),
    ];
    let ignores_contacts = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    if !ignores_contacts && mode & MODE_FAMILY != FLIGHT_MODE {
        world
            .player_mut(objects, owner)?
            .occupancy
            .as_mut()
            .expect("validated occupancy")
            .displacement = delta;
    }
    // Resolve history at its first write. A missing owner may leave the X
    // position prefix committed, exactly as the other scene services do.
    let x = position.x.wrapping_add(delta[0]);
    objects
        .get_mut(owner)
        .expect("validated player")
        .base
        .position
        .x = x;
    world
        .player_mut(objects, owner)?
        .boundary
        .as_mut()
        .ok_or(OccupancyError::MissingBoundary(owner))?
        .return_position
        .x = x;
    let z = position.z.wrapping_add(delta[1]);
    objects
        .get_mut(owner)
        .expect("validated player")
        .base
        .position
        .z = z;
    world
        .player_mut(objects, owner)?
        .boundary
        .as_mut()
        .expect("validated boundary")
        .return_position
        .z = z;
    Ok(())
}

#[cfg(test)]
#[path = "player_occupancy_tests.rs"]
mod tests;
