use sf2_data::{collision_data::collision_profile, path::PathAddress, shape_data::shape_by_id};
use sf2_path::{
    ChildSpawn, ContextTransition, ObjectSpawn, PathContactClass, PathTrigger, PlayerTargetUpdate,
    SelectedMarkerClass, Sf2PathCondition, Sf2PathHost, Sf2PathOperation,
};

use crate::object::*;
use crate::oracle_compat::{Error, Game, MapMarker};

const COPROCESSOR_CONTROL_SHADOW: u16 = 0x005E;
const ENABLED_CONTROL_BITS: u8 = 0xE7;
const CAPTURE_ANGLE_SCRATCH: u16 = 0x00A7;
const CAPTURE_X_SCRATCH: u16 = 0x0002;
const CAPTURE_Y_SCRATCH: u16 = 0x0008;
const CAPTURE_Z_SCRATCH: u16 = 0x0097;
const CAPTURE_PARAMETER_X_SCRATCH: u16 = 0x0004;
const CAPTURE_PARAMETER_Y_SCRATCH: u16 = 0x000A;
const CAPTURE_RANGE_SCRATCH: u16 = 0x00E4;
const CAPTURE_PARAMETER_X_STATE: u16 = 0x16B7;
const CAPTURE_PARAMETER_Y_STATE: u16 = 0x16B9;
const CAPTURE_RANGE_STATE: u16 = 0x16BB;
const CAPTURE_ANGLE_STATE: u16 = 0x16BF;
const AUXILIARY_CAPTURE_FLAGS: u16 = 0x6B77;
const AUXILIARY_CAPTURE_ANGLE: u16 = 0x6AAE;
const AUXILIARY_CAPTURE_X: u16 = 0x6AAF;
const AUXILIARY_CAPTURE_Y: u16 = 0x6AB1;
const AUXILIARY_CAPTURE_Z: u16 = 0x6AB3;
const AUXILIARY_CAPTURE_PARAMETER_X: u16 = 0x6AB5;
const AUXILIARY_CAPTURE_PARAMETER_Y: u16 = 0x6AB7;
const AUXILIARY_CAPTURED_FLAG: u8 = 0x04;
const CAPTURE_ANGLE_GROUP_MASK: u8 = 0xE0;
const CONTACT_TARGET_SCRATCH: u16 = 0x0008;
const CONTACT_VALUE_SCRATCH: u16 = 0x0002;
const CONTACT_ALTERNATE_CLASS_STATE: u16 = 0xD746;
const CONTACT_FRAME_STATE: u16 = 0x1B4D;
const CONTACT_FRAME_MASK: u8 = 0x07;
const CONTACT_VERTICAL_MARGIN: i16 = 20;
const COLLISION_TARGET_EXTENSION: u16 = 0x1CE8;
const COLLISION_BOX_EXTENSION: u16 = 0x1CEA;
const COLLISION_FLAGS_EXTENSION: u16 = 0x1CEB;
const COLLISION_NEAREST_SURFACE: u16 = 0x195D;
const COLLISION_NEAREST_TARGET: u16 = 0x195F;
const COLLISION_NEAREST_BOX: u16 = 0x1961;
const COLLISION_NEAREST_FLAGS: u16 = 0x1A8D;
const COLLISION_ANIMATION_FRAME_EXTENSION: u16 = 0x1CCB;
const COLLISION_GLOBAL_FRAME: u16 = 0x00C4;
const COLLISION_FULL_SEARCH_LIMIT: u16 = 16_384;
const COLLISION_REDUCED_SEARCH_LIMIT: u16 = 8_192;
const COLLISION_CANDIDATE_DISABLED_FLAG_24: u8 = 0x04;
const COLLISION_CANDIDATE_DISABLED_FLAG_31: u8 = 0x04;
const COLLISION_REACTIVE_CURRENT_FLAGS: u8 = 0x88;
const COLLISION_TARGET_FLAG_22: u8 = 0x02;
const COLLISION_TARGET_FLAG_26: u8 = 0x01;
const COLLISION_TARGET_SIDE_FLAG_26: u8 = 0x02;
const COLLISION_TARGET_ALTERNATE_SIDE_FLAG_26: u8 = 0x04;
const COLLISION_CURRENT_ALTERNATE_SIDE_FLAG_24: u8 = 0x80;
const DIRECT_CONTACT_FLAG_20: u8 = 0x80;
const DIRECT_CONTACT_FLAG_21: u8 = 0x02;
const ALTERNATE_CONTACT_FLAG_22: u8 = 0x08;
const OBJECT_HEALTH_SNAPSHOT_FIELD: u16 = 0x27;
const OBJECT_HIT_POINTS_FIELD: u16 = 0x2D;
const OBJECT_VERTICAL_VELOCITY_FIELD: u16 = 0x34;
const OBJECT_HORIZONTAL_MOTION_FIELD: u16 = 0x32;
const OBJECT_DEPTH_MOTION_FIELD: u16 = 0x36;
const OBJECT_LIFETIME_FIELD: u16 = 0x18;
const OBJECT_VARIANT_FIELD: u16 = 0x2E;
const OBJECT_AUXILIARY_BYTE_FIELD: u16 = 0x0A;
const OBJECT_RELATIVE_X_EXTENSION: u16 = 0x1CCF;
const OBJECT_RELATIVE_Y_EXTENSION: u16 = 0x1CD1;
const OBJECT_RELATIVE_Z_EXTENSION: u16 = 0x1CD3;
const OBJECT_RELATIVE_PITCH_EXTENSION: u16 = 0x1CD5;
const OBJECT_RELATIVE_YAW_EXTENSION: u16 = 0x1CD6;
const OBJECT_RELATIVE_ROLL_EXTENSION: u16 = 0x1CD7;
const OBJECT_MOTION_PHASE_EXTENSION: u16 = 0x1CE2;
const PLAYER_ALTERNATE_HORIZONTAL_MOTION_EXTENSION: u16 = 0x1CC1;
const PLAYER_ALTERNATE_DEPTH_MOTION_EXTENSION: u16 = 0x1CC5;
const FIXED_PLAYER_OBJECT: u16 = 0x033F;
const DAMAGE_SCRATCH: u16 = 0x003A;
const DAMAGE_GLOBAL_MIRROR: u16 = 0xD773;
const PLAYER_YAW_DAMPING_STATE: u16 = 0x1E52;
const PLAYER_YAW_DAMPING_SCRATCH: u16 = 0x003C;
const RANDOM_VARIANT_STATE: u16 = 0x1D8F;
const PLAYER_AUXILIARY_MODE_BASE: u16 = 0x6AA0;
const PLAYER_AUXILIARY_CHARGE_BASE: u16 = 0x6BE2;
const PLAYER_AUXILIARY_CONTROL_BASE: u16 = 0x6A8C;
const PLAYER_AUXILIARY_CONTROL_X_BASE: u16 = 0x6C2A;
const PLAYER_AUXILIARY_CONTROL_Y_BASE: u16 = 0x6C2B;
const PLAYER_AUXILIARY_CONTROL_Z_BASE: u16 = 0x6C2C;
const PLAYER_AUXILIARY_RATE_X_BASE: u16 = 0x6A8D;
const PLAYER_AUXILIARY_RATE_Y_BASE: u16 = 0x6A8E;
const PLAYER_AUXILIARY_RATE_Z_BASE: u16 = 0x6A8F;
const PLAYER_AUXILIARY_RANGE_BASE: u16 = 0x6A90;
const PLAYER_AUXILIARY_SECONDARY_RANGE_BASE: u16 = 0x6C26;
const PLAYER_AUXILIARY_LINK_FLAGS_BASE: u16 = 0x6B63;
const PLAYER_AUXILIARY_LINK_MODE_BASE: u16 = 0x6C02;
const PLAYER_AUXILIARY_TRANSFORM_MODE_BASE: u16 = 0x6C08;
const PLAYER_AUXILIARY_CHARGE_GLOBAL_FLAGS: u16 = 0x1D74;
const PLAYER_AUXILIARY_REFRESH_CLASS: u16 = 0x1DE2;
const PLAYER_AUXILIARY_REFRESH_GATE: u16 = 0x1D72;
const PLAYER_AUXILIARY_REFRESH_OVERRIDE: u16 = 0x1E0D;
const PLAYER_AUXILIARY_REFRESH_DISABLE: u16 = 0xD7F4;
const PLAYER_AUXILIARY_REFRESH_FRAME: u16 = 0x1B4D;
const OBJECT_LINKED_OBJECT_FIELD: u16 = 0x06;
const OBJECT_SELECTED_LINK_FLAG_EXTENSION: u16 = 0x1CDA;
const OBJECT_RELATIVE_REFERENCE_EXTENSION: u16 = 0x1CD8;
const OBJECT_RELATIVE_DEPTH_SOURCE_EXTENSION: u16 = 0x1CE4;
const OBJECT_CONDITIONAL_PHASE_EXTENSION: u16 = 0x1CC8;
const CONDITIONAL_OBJECT_PHASE_STATE: u16 = 0x1DD1;
const CONDITIONAL_OBJECT_CONTROL_FLAGS: u16 = 0x00C4;
const DAMAGE_CLAMP_THRESHOLD: u8 = 3;
const DAMAGE_CLAMP_AMOUNT: u8 = 4;
const YAW_SEPARATION_STEP: u8 = 30;
const VERTICAL_OSCILLATION_CENTER: i16 = 1_500;
const VERTICAL_OSCILLATION_SPAN: u16 = 3_000;
const RANDOMIZED_OBJECT_LIFETIME: u8 = 50;
const RANDOMIZED_OBJECT_VARIANT_MASK: u8 = 7;
const PLAYER_AUXILIARY_MODE_MASK: u8 = 0xF0;
const PLAYER_DIRECT_MOTION_MODE: u8 = 0x10;
const PLAYER_AUXILIARY_SELECTED_FLAG: u8 = 0x80;
const PLAYER_AUXILIARY_CHARGE_VALUE: u8 = 63;
const PLAYER_AUXILIARY_CHARGE_ENABLE_BITS: u8 = 0x05;
const PLAYER_AUXILIARY_CHARGE_ACTIVE_BIT: u8 = 0x04;
const PLAYER_AUXILIARY_CONTROL_VALUE: u8 = 16;
const PLAYER_AUXILIARY_CONTROL_DEPTH_VALUE: u8 = 31;
const PLAYER_AUXILIARY_RATE_X_VALUE: u8 = 3;
const PLAYER_AUXILIARY_RATE_Y_VALUE: u8 = 3;
const PLAYER_AUXILIARY_RATE_Z_VALUE: u8 = 4;
const PLAYER_AUXILIARY_RANGE_VALUE: u16 = 8;
const PLAYER_AUXILIARY_REFRESH_CLASS_VALUE: u8 = 9;
const PLAYER_AUXILIARY_REFRESH_FRAME_MASK: u8 = 7;
const PLAYER_AUXILIARY_REFRESH_OVERRIDE_BIT: u8 = 0x01;
const PLAYER_AUXILIARY_MODE_VALUE_MASK: u8 = 0x1F;
const PLAYER_AUXILIARY_MODE_ACTIVITY_MASK: u8 = 0xFE;
const PLAYER_AUXILIARY_MODE_STICKY_BIT: u8 = 0x20;
const PLAYER_AUXILIARY_NORMALIZED_MODE: u8 = 1;
const PLAYER_AUXILIARY_PITCH_STEP: u8 = 8;
const PLAYER_AUXILIARY_ROLL_STEP: u8 = 6;
const RELATIVE_OFFSET_CHASE_TARGET: i16 = 200;
const RELATIVE_OFFSET_ANGLE_STEP: u8 = 8;
const RELATIVE_OFFSET_POSITION_STEP: u16 = 10;
const CONDITIONAL_OBJECT_PHASE_LIMIT: u8 = 13;
const CONDITIONAL_OBJECT_ACTIVE_PHASE: u8 = 1;
const CONDITIONAL_OBJECT_DELAY: u16 = 3;
const CONDITIONAL_OBJECT_CONTROL_MASK: u8 = 0x05;
const PLAYER_RELATIVE_DEPTH_OFFSET: i16 = 80;
const SPAWNED_OBJECT_VERTICAL_OFFSET: i16 = 10;
const SPAWNED_OBJECT_DEPTH_OFFSET: i16 = 80;
const SPAWNED_OBJECT_OFFSET_SCALE: u32 = 6;
const SPAWNED_OBJECT_VARIANT_DEPTH: [i16; 8] = [-15, -10, 0, 10, 20, 10, 0, -10];
const SPAWNED_OBJECT_YAW_STATE: u16 = 0x1BA9;
const RANDOMIZED_OBJECT_POSITION_X: [i16; 8] = [150, -450, -450, -100, 100, 450, 450, -150];
const RANDOMIZED_OBJECT_POSITION_Y: [i16; 8] = [50, -50, 0, -150, -150, 0, -50, 50];
const RANDOMIZED_OBJECT_POSITION_Z: [i16; 8] = [1_200, 200, -400, -700, -700, -400, 200, 1_200];
const RANDOMIZED_OBJECT_ROTATION_X: [u8; 8] = [12, 16, 12, 12, 12, 12, 16, 12];
const RANDOMIZED_OBJECT_ROTATION_Y: [u8; 8] = [128, 168, 224, 0, 0, 32, 88, 128];
const RANDOMIZED_OBJECT_ROTATION_Z: [u8; 8] = [0, 236, 226, 216, 40, 30, 20, 0];
const RANDOMIZED_OBJECT_RELATIVE_PITCH: [u8; 8] = [12, 16, 18, 32, 32, 18, 16, 12];
const RANDOMIZED_OBJECT_MOTION_PHASE: [u8; 8] = [0, 254, 252, 252, 4, 4, 2, 0];
const RANDOMIZED_OBJECT_RELATIVE_ROLL: [u8; 8] = [0; 8];
const RANDOMIZED_OBJECT_AUXILIARY: [u8; 8] = [20, 12, 7, 8, 8, 7, 12, 20];
const CONTACT_LINK_FIELD: u16 = 0x1E;
const CONTACT_LINK_TARGET_FIELD: u16 = 4;
const ORDINARY_CONTACT_AUXILIARY_KIND: u8 = 0x0B;
const ALTERNATE_CONTACT_AUXILIARY_KIND: u8 = 0x0D;
const LAST_SPAWNED_OBJECT: u16 = 0xD771;
const SPAWNED_OBJECT_OWNER_FIELD: u16 = 0x1C;
const OBJECT_FLAG_25_FIELD: u16 = 0x25;
const PLAYER_ONE_TRANSFORM_LOCKED_FLAG: u8 = 0x20;
const LAUNCHED_EXTERNAL_OBJECT: u16 = 0x14D6;
const LAUNCHED_EXTERNAL_STEP_COUNTER: u16 = 0x16B1;
const LAUNCHED_EXTERNAL_POSITION_X: u16 = 0xD767;
const LAUNCHED_EXTERNAL_POSITION_Z: u16 = 0xD769;
const LAUNCHED_EXTERNAL_SPEED: u8 = 50;
const LAUNCHED_EXTERNAL_CLEARANCE: u16 = 16;
const LAUNCHED_EXTERNAL_STEP_LIMIT: u8 = 20;
const PLAYER_LINKED_OBJECT_SHAPE: u16 = 0xC5CC;
const PLAYER_LINKED_OBJECT_STRATEGY: u16 = 0xF9BA;
const PLAYER_LINKED_OBJECT_STRATEGY_BANK: u8 = 0x06;
const PLAYER_LINKED_OBJECT_ACTIVE_MODE_MASK: u8 = 0x1F;
const PLAYER_LINKED_OBJECT_PARAMETER_FIELD: u16 = 0x13;
const PLAYER_LINKED_OBJECT_PARAMETER: u8 = 12;
const PLAYER_LINKED_OBJECT_ACTIVITY_STATE: u16 = 0x1DDF;
const PLAYER_LINKED_OBJECT_EXTENSION_STATE: u16 = 0x1CF0;
const PLAYER_LINKED_OBJECT_HIT_POINTS: u8 = 1;
const PLAYER_LINKED_OBJECT_ATTACK_POINTS: u8 = 1;
const PLAYER_LINKED_OBJECT_FLAG_21: u8 = 0x01;
const PLAYER_LINKED_OBJECT_FLAG_22: u8 = 0x04;
const PLAYER_LINKED_OBJECT_FLAG_23: u8 = 0x04;
const PLAYER_LINKED_OBJECT_FLAG_25: u8 = 0x01;
const PLAYER_LINKED_OBJECT_FLAG_26: u8 = 0x08;
const PLAYER_LINKED_OBJECT_CLASS_MASK: u8 = 0xEF;
const PLAYER_AUXILIARY_TARGET_DELAY_BASE: u16 = 0x6BEA;
const PLAYER_AUXILIARY_TARGET_OWNER_BASE: u16 = 0x6A98;
const PLAYER_AUXILIARY_TARGET_MODE_BASE: u16 = 0x6C1C;
const PLAYER_AUXILIARY_TARGET_LIMIT_BASE: u16 = 0x6C24;
const PLAYER_AUXILIARY_TARGET_SECONDARY_LIMIT_BASE: u16 = 0x6C26;
const PLAYER_AUXILIARY_TARGET_CONTROL_BASE: u16 = 0x6C28;
const PLAYER_AUXILIARY_TARGET_AXIS_BASE: u16 = 0x6C29;
const PLAYER_AUXILIARY_TARGET_DELAY: u8 = 10;
const PLAYER_AUXILIARY_TARGET_MODE: u16 = 2;
const PLAYER_AUXILIARY_TARGET_LIMIT: u16 = 255;
const PLAYER_AUXILIARY_TARGET_AXIS: u8 = 3;
const PLAYER_AUXILIARY_TARGET_CONTROL: u8 = 31;
const PLAYER_AUXILIARY_TARGET_RATE_X: u8 = 3;
const PLAYER_AUXILIARY_TARGET_RATE_Y: u8 = 3;
const PLAYER_AUXILIARY_TARGET_RATE_Z: u8 = 2;
const PLAYER_AUXILIARY_TARGET_RANGE_X: u8 = 25;
const PLAYER_AUXILIARY_TARGET_RANGE_Y: u8 = 25;
const PLAYER_AUXILIARY_TARGET_RANGE_Z: u8 = 31;
const PLAYER_AUXILIARY_CONFIGURATION_LOCK: u8 = 0x40;
const CAPTURE_EXTERNAL_OBJECT: u16 = 0x14D6;
const CAPTURE_BOUNDARY_CENTER_X: u16 = 0x1DBC;
const CAPTURE_BOUNDARY_CENTER_Z: u16 = 0x1DB8;
const CAPTURE_BOUNDARY_ANGLE: u16 = 0x1DB2;
const CAPTURE_BOUNDARY_RADIUS: u16 = 0x1DB6;
const CAPTURE_BOUNDARY_UPPER: u16 = 0x1DBE;
const CAPTURE_BOUNDARY_LOWER: u16 = 0x1DC0;
const CAPTURE_DIAGONAL_SCALE: i32 = 362;
const CAPTURE_EXTERNAL_CONTACT_BIT: u8 = 0x02;

#[derive(Clone, Copy)]
enum PilotAuxiliaryMode {
    DoubledValue,
    AlternateAxes,
    FullControl,
}

#[derive(Clone, Copy)]
enum CaptureBoundaryOrientation {
    HorizontalBand,
    DepthBand,
    RisingDiagonalBand,
    RisingDiagonalPositiveSide,
    FallingDiagonalBand,
    FallingDiagonalNegativeSide,
}

impl CaptureBoundaryOrientation {
    fn from_angle(angle: u8) -> Self {
        match angle {
            64 | 192 => Self::DepthBand,
            32 => Self::RisingDiagonalBand,
            160 => Self::RisingDiagonalPositiveSide,
            96 => Self::FallingDiagonalBand,
            224 => Self::FallingDiagonalNegativeSide,
            _ => Self::HorizontalBand,
        }
    }
}

impl Game {
    fn suspend_coprocessor_interrupts(&mut self) {
        let control = self.memory.read_byte(COPROCESSOR_CONTROL_SHADOW) & ENABLED_CONTROL_BITS;
        self.memory.write_byte(COPROCESSOR_CONTROL_SHADOW, control);
    }

    fn wrapping_absolute_difference(left: i16, right: i16) -> u16 {
        let difference = left.wrapping_sub(right);
        if difference < 0 {
            difference.wrapping_neg() as u16
        } else {
            difference as u16
        }
    }

    fn collision_axis_contains(center: u16, span: u16, point: u16) -> bool {
        center.wrapping_add(span).wrapping_sub(point) < span.wrapping_mul(2)
    }

    fn collision_footprint_contains(center: i16, size: u16, point: i16) -> bool {
        (size >> 1)
            .wrapping_add(center as u16)
            .wrapping_add(1)
            .wrapping_sub(point as u16)
            < size
    }

    /// Refresh the downward contact projection used by the three-way path
    /// branch. ShapeHdr bounds cover ordinary colliders; the complete typed
    /// compound-profile catalog covers animated, rotated, polygon-clipped
    /// collision planes. Only the fixed-point math kernels remain in the
    /// feature-gated retail oracle.
    fn refresh_collision_projection(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let current_x = self.memory.read_word(current.wrapping_add(FIELD_X));
        let current_y = self.memory.read_word(current.wrapping_add(FIELD_Y));
        let current_z = self.memory.read_word(current.wrapping_add(FIELD_Z));
        let reduced_search = self.memory.read_byte(CONTACT_FRAME_STATE) & CONTACT_FRAME_MASK != 0;
        let initial_surface = if reduced_search {
            COLLISION_REDUCED_SEARCH_LIMIT
        } else {
            COLLISION_FULL_SEARCH_LIMIT
        };
        let mut nearest_surface = initial_surface;
        let mut nearest_target = 0;
        let mut nearest_box = 0;
        let mut nearest_flags = 0;

        for candidate in active_objects(&self.memory) {
            if candidate == current
                || self.memory.read_byte(candidate.wrapping_add(0x31))
                    & COLLISION_CANDIDATE_DISABLED_FLAG_31
                    != 0
                || self.memory.read_byte(candidate.wrapping_add(0x24))
                    & COLLISION_CANDIDATE_DISABLED_FLAG_24
                    != 0
            {
                continue;
            }

            let shape_id = self.memory.read_word(candidate.wrapping_add(FIELD_SHAPE));
            let Some(shape) = shape_by_id(shape_id) else {
                continue;
            };
            let [span_x, span_y, span_z] = shape.bounds;
            if !Self::collision_axis_contains(
                self.memory.read_word(candidate.wrapping_add(FIELD_X)),
                span_x,
                current_x,
            ) || !Self::collision_axis_contains(
                self.memory.read_word(candidate.wrapping_add(FIELD_Z)),
                span_z,
                current_z,
            ) {
                continue;
            }

            let candidate_y = self.memory.read_word(candidate.wrapping_add(FIELD_Y));
            if let Some(profile) = collision_profile(shape_id) {
                let relative_x = current_x
                    .wrapping_sub(self.memory.read_word(candidate.wrapping_add(FIELD_X)))
                    as i16;
                let relative_z = current_z
                    .wrapping_sub(self.memory.read_word(candidate.wrapping_add(FIELD_Z)))
                    as i16;
                let yaw = self.memory.read_byte(candidate.wrapping_add(FIELD_ROT_Y));
                let (local_x, local_z) = if yaw == 0 {
                    (relative_x, relative_z)
                } else {
                    self.rotate_collision_probe(yaw, relative_x, relative_z)
                };
                let frame_marker = self
                    .memory
                    .read_byte(candidate.wrapping_add(COLLISION_ANIMATION_FRAME_EXTENSION));
                let frame = if frame_marker & 0x80 != 0 {
                    usize::from(frame_marker & 0x7F)
                } else {
                    usize::from(self.memory.read_byte(COLLISION_GLOBAL_FRAME))
                };

                for (group_index, group) in profile.groups.iter().enumerate() {
                    let record = group.variants.get(frame).unwrap_or(&group.variants[0]);
                    if !Self::collision_footprint_contains(record.center_x, record.width, local_x)
                        || !Self::collision_footprint_contains(
                            record.center_z,
                            record.depth,
                            local_z,
                        )
                    {
                        continue;
                    }
                    if let Some(polygon) = record.polygon {
                        if !self.collision_polygon_contains(
                            polygon.source_address,
                            polygon.scale,
                            local_x,
                            local_z,
                        ) {
                            continue;
                        }
                    }

                    let surface = candidate_y.wrapping_add(self.project_collision_surface(
                        record.plane_normal,
                        record.plane_offset,
                        local_x,
                        local_z,
                    ) as u16);
                    let upper_edge = surface.wrapping_add(span_y).wrapping_add(2);
                    if (upper_edge.wrapping_sub(current_y) as i16) < 0
                        || (surface.wrapping_sub(nearest_surface) as i16) >= 0
                    {
                        continue;
                    }
                    nearest_surface = surface;
                    nearest_target = candidate;
                    nearest_box = (profile.groups.len() - group_index) as u16;
                    nearest_flags = u16::from(record.box_flags);
                }
            } else {
                let surface = candidate_y.wrapping_sub(span_y);
                let upper_edge = surface.wrapping_add(span_y).wrapping_add(2);
                if (upper_edge.wrapping_sub(current_y) as i16) < 0
                    || (surface.wrapping_sub(nearest_surface) as i16) >= 0
                {
                    continue;
                }
                nearest_surface = surface;
                nearest_target = candidate;
                nearest_box = 0;
                nearest_flags = 0;
            }
        }

        let projected_surface = if nearest_surface == COLLISION_REDUCED_SEARCH_LIMIT {
            0
        } else {
            nearest_surface
        };
        self.memory
            .write_word(CONTACT_TARGET_SCRATCH, projected_surface);
        self.memory.write_word(
            current.wrapping_add(COLLISION_TARGET_EXTENSION),
            nearest_target,
        );
        self.memory.write_byte(
            current.wrapping_add(COLLISION_BOX_EXTENSION),
            nearest_box as u8,
        );
        self.memory.write_byte(
            current.wrapping_add(COLLISION_FLAGS_EXTENSION),
            nearest_flags as u8,
        );
        self.memory
            .write_word(COLLISION_NEAREST_SURFACE, nearest_surface);
        self.memory
            .write_word(COLLISION_NEAREST_TARGET, nearest_target);
        self.memory.write_word(COLLISION_NEAREST_BOX, nearest_box);
        self.memory
            .write_word(COLLISION_NEAREST_FLAGS, nearest_flags);
        Ok(())
    }

    fn capture_selected_auxiliary_motion(&mut self) -> Result<(), Error> {
        self.suspend_coprocessor_interrupts();
        let current = self.current_object()?;
        let selected = self.selected_object().ok_or(Error::InvalidObject(0))?;
        let slot = self.memory.read_word(selected + FIELD_PATH);

        let angle = (self.memory.read_byte(current + FIELD_ROT_Y) & CAPTURE_ANGLE_GROUP_MASK)
            .wrapping_add(self.memory.read_byte(CAPTURE_ANGLE_SCRATCH));
        self.memory.write_byte(CAPTURE_ANGLE_SCRATCH, angle);
        self.memory.write_byte(CAPTURE_ANGLE_STATE, angle);

        let position = [
            self.memory.read_word(current + FIELD_X),
            self.memory.read_word(current + FIELD_Y),
            self.memory.read_word(current + FIELD_Z),
        ];
        for (scratch, value) in [
            (CAPTURE_X_SCRATCH, position[0]),
            (CAPTURE_Y_SCRATCH, position[1]),
            (CAPTURE_Z_SCRATCH, position[2]),
        ] {
            self.memory.write_word(scratch, value);
        }

        let parameter_x = self.memory.read_word(CAPTURE_PARAMETER_X_SCRATCH);
        let parameter_y = self.memory.read_word(CAPTURE_PARAMETER_Y_SCRATCH);
        let range = self.memory.read_word(CAPTURE_RANGE_SCRATCH);
        self.memory
            .write_word(CAPTURE_PARAMETER_X_STATE, parameter_x);
        self.memory
            .write_word(CAPTURE_PARAMETER_Y_STATE, parameter_y);
        self.memory.write_word(CAPTURE_RANGE_STATE, range);

        let flags_address = AUXILIARY_CAPTURE_FLAGS.wrapping_add(slot);
        let already_captured = self.memory.read_byte(flags_address) & AUXILIARY_CAPTURED_FLAG != 0;
        let eligible = if already_captured {
            true
        } else {
            self.selected_auxiliary_capture_is_eligible(selected)
        };

        let within_x = Self::wrapping_absolute_difference(
            position[0] as i16,
            self.memory.read_word(selected + FIELD_X) as i16,
        ) < range;
        let within_z = Self::wrapping_absolute_difference(
            position[2] as i16,
            self.memory.read_word(selected + FIELD_Z) as i16,
        ) < range;
        if eligible && within_x && within_z {
            let flags = self.memory.read_byte(flags_address) | AUXILIARY_CAPTURED_FLAG;
            self.memory.write_byte(flags_address, flags);
            self.memory
                .write_byte(AUXILIARY_CAPTURE_ANGLE.wrapping_add(slot), angle);
            for (base, value) in [
                (AUXILIARY_CAPTURE_X, position[0]),
                (AUXILIARY_CAPTURE_Y, position[1]),
                (AUXILIARY_CAPTURE_Z, position[2]),
                (AUXILIARY_CAPTURE_PARAMETER_X, parameter_x),
                (AUXILIARY_CAPTURE_PARAMETER_Y, parameter_y),
            ] {
                self.memory.write_word(base.wrapping_add(slot), value);
            }
        }

        for address in [
            CAPTURE_PARAMETER_X_STATE,
            CAPTURE_PARAMETER_Y_STATE,
            CAPTURE_RANGE_STATE,
        ] {
            self.memory.write_word(address, 0);
        }
        Ok(())
    }

    fn selected_auxiliary_capture_is_eligible(&mut self, selected: u16) -> bool {
        let vertical_radius = self.memory.read_word(CAPTURE_PARAMETER_Y_SCRATCH);
        if vertical_radius as i16 >= 0 {
            let center = self.memory.read_word(CAPTURE_Y_SCRATCH);
            let selected_y = self.memory.read_word(selected.wrapping_add(FIELD_Y));
            let below = center
                .wrapping_sub(vertical_radius)
                .wrapping_sub(selected_y);
            if below as i16 >= 0 {
                return false;
            }
            let above = center
                .wrapping_add(vertical_radius)
                .wrapping_sub(selected_y);
            if (above as i16) < 0 {
                return false;
            }
        }

        let center_x = self.memory.read_word(CAPTURE_X_SCRATCH);
        let center_z = self.memory.read_word(CAPTURE_Z_SCRATCH);
        let angle = self.memory.read_byte(CAPTURE_ANGLE_SCRATCH);
        let radius = self.memory.read_word(CAPTURE_PARAMETER_X_SCRATCH);
        self.memory.write_word(CAPTURE_BOUNDARY_CENTER_X, center_x);
        self.memory.write_word(CAPTURE_BOUNDARY_CENTER_Z, center_z);
        self.memory
            .write_word(CAPTURE_BOUNDARY_ANGLE, u16::from(angle));
        self.memory.write_word(CAPTURE_BOUNDARY_RADIUS, radius);

        let external = self.memory.read_word(CAPTURE_EXTERNAL_OBJECT);
        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            let value = self.memory.read_word(selected.wrapping_add(field));
            self.memory.write_word(external.wrapping_add(field), value);
        }
        let flags =
            self.memory.read_byte(external.wrapping_add(0x20)) & !CAPTURE_EXTERNAL_CONTACT_BIT;
        self.memory.write_byte(external.wrapping_add(0x20), flags);

        let mut external_x = self.memory.read_word(external.wrapping_add(FIELD_X));
        let mut external_z = self.memory.read_word(external.wrapping_add(FIELD_Z));
        let diagonal_radius = ((i32::from(radius as i16) * CAPTURE_DIAGONAL_SCALE) >> 8) as u16;

        let eligible = match CaptureBoundaryOrientation::from_angle(angle) {
            CaptureBoundaryOrientation::HorizontalBand => {
                let upper_delta = center_x.wrapping_add(radius).wrapping_sub(external_x);
                if (upper_delta as i16) < 0 {
                    external_x = external_x.wrapping_add(upper_delta);
                    false
                } else {
                    let lower_delta = center_x.wrapping_sub(radius).wrapping_sub(external_x);
                    if (lower_delta as i16) < 0 {
                        true
                    } else {
                        external_x = external_x.wrapping_add(lower_delta);
                        false
                    }
                }
            }
            CaptureBoundaryOrientation::DepthBand => {
                let upper_delta = center_z.wrapping_add(radius).wrapping_sub(external_z);
                if (upper_delta as i16) < 0 {
                    external_z = external_z.wrapping_add(upper_delta);
                    false
                } else {
                    let lower_delta = center_z.wrapping_sub(radius).wrapping_sub(external_z);
                    if (lower_delta as i16) < 0 {
                        true
                    } else {
                        external_z = external_z.wrapping_add(lower_delta);
                        false
                    }
                }
            }
            CaptureBoundaryOrientation::RisingDiagonalBand => {
                let center = center_x.wrapping_add(center_z);
                let upper = center.wrapping_add(diagonal_radius);
                let lower = center.wrapping_sub(diagonal_radius);
                self.memory.write_word(CAPTURE_BOUNDARY_UPPER, upper);
                self.memory.write_word(CAPTURE_BOUNDARY_LOWER, lower);
                let position = external_x.wrapping_add(external_z);
                let upper_delta = position.wrapping_sub(upper);
                if upper_delta as i16 >= 0 {
                    let correction = ((upper_delta.wrapping_neg() as i16) >> 1) as u16;
                    external_x = external_x.wrapping_add(correction);
                    external_z = external_z.wrapping_add(correction);
                    false
                } else {
                    let lower_delta = position.wrapping_sub(lower);
                    if lower_delta as i16 >= 0 {
                        true
                    } else {
                        let correction = ((lower_delta.wrapping_neg() as i16) >> 1) as u16;
                        external_x = external_x.wrapping_add(correction);
                        external_z = external_z.wrapping_add(correction);
                        false
                    }
                }
            }
            CaptureBoundaryOrientation::RisingDiagonalPositiveSide => {
                let center = center_x.wrapping_add(center_z);
                let upper = center.wrapping_add(diagonal_radius);
                let lower = center.wrapping_sub(diagonal_radius);
                self.memory.write_word(CAPTURE_BOUNDARY_UPPER, upper);
                self.memory.write_word(CAPTURE_BOUNDARY_LOWER, lower);
                let position = external_x.wrapping_add(external_z);
                let lower_delta = position.wrapping_sub(lower);
                if (lower_delta as i16) < 0 {
                    let correction = ((lower_delta.wrapping_neg() as i16) >> 1) as u16;
                    external_x = external_x.wrapping_add(correction);
                    external_z = external_z.wrapping_add(correction);
                    false
                } else {
                    let upper_delta = position.wrapping_sub(upper);
                    if upper_delta as i16 >= 0 {
                        let correction = ((upper_delta.wrapping_neg() as i16) >> 1) as u16;
                        external_x = external_x.wrapping_add(correction);
                        external_z = external_z.wrapping_add(correction);
                        false
                    } else {
                        true
                    }
                }
            }
            CaptureBoundaryOrientation::FallingDiagonalBand => {
                let upper = center_x
                    .wrapping_sub(center_z)
                    .wrapping_add(diagonal_radius);
                let negative_lower = center_z
                    .wrapping_sub(center_x)
                    .wrapping_add(diagonal_radius);
                self.memory
                    .write_word(CAPTURE_BOUNDARY_UPPER, negative_lower);
                self.memory.write_word(CAPTURE_BOUNDARY_LOWER, upper);
                let lower_delta = external_x
                    .wrapping_sub(external_z)
                    .wrapping_add(negative_lower);
                if (lower_delta as i16) < 0 {
                    let correction = ((lower_delta as i16) >> 1) as u16;
                    external_z = external_z.wrapping_add(correction);
                    external_x = external_x.wrapping_sub(correction);
                    false
                } else {
                    let upper_delta = external_z.wrapping_sub(external_x).wrapping_add(upper);
                    if upper_delta as i16 >= 0 {
                        true
                    } else {
                        let correction = ((upper_delta as i16) >> 1) as u16;
                        external_x = external_x.wrapping_add(correction);
                        external_z = external_z.wrapping_sub(correction);
                        false
                    }
                }
            }
            CaptureBoundaryOrientation::FallingDiagonalNegativeSide => {
                let upper = center_x
                    .wrapping_sub(center_z)
                    .wrapping_add(diagonal_radius);
                let negative_lower = center_z
                    .wrapping_sub(center_x)
                    .wrapping_add(diagonal_radius);
                self.memory
                    .write_word(CAPTURE_BOUNDARY_UPPER, negative_lower);
                self.memory.write_word(CAPTURE_BOUNDARY_LOWER, upper);
                let upper_delta = external_x.wrapping_sub(external_z).wrapping_sub(upper);
                if upper_delta as i16 >= 0 {
                    let correction = ((upper_delta as i16) >> 1) as u16;
                    external_z = external_z.wrapping_add(correction);
                    external_x = external_x.wrapping_sub(correction);
                    false
                } else {
                    let lower_delta = external_z
                        .wrapping_sub(external_x)
                        .wrapping_sub(negative_lower);
                    if lower_delta as i16 >= 0 {
                        let correction = ((lower_delta as i16) >> 1) as u16;
                        external_x = external_x.wrapping_add(correction);
                        external_z = external_z.wrapping_sub(correction);
                        false
                    } else {
                        true
                    }
                }
            }
        };

        self.memory
            .write_word(external.wrapping_add(FIELD_X), external_x);
        self.memory
            .write_word(external.wrapping_add(FIELD_Z), external_z);
        eligible
    }

    fn classify_contact_target(&mut self, target: u16) -> PathContactClass {
        self.memory.write_word(CONTACT_VALUE_SCRATCH, 0);
        if target == 0 {
            return PathContactClass::NoObject;
        }

        let selector = if target == self.memory.read_word(PLAYER_ONE)
            || target == self.memory.read_word(PLAYER_TWO)
            || self
                .memory
                .read_byte(target.wrapping_add(OBJECT_HIT_POINTS_FIELD))
                == 0
        {
            target as u8
        } else if self.memory.read_byte(target.wrapping_add(0x22)) & ALTERNATE_CONTACT_FLAG_22 != 0
        {
            if let Some(entry) =
                find_auxiliary_type(&self.memory, target, ALTERNATE_CONTACT_AUXILIARY_KIND)
            {
                self.memory.write_byte(
                    CONTACT_VALUE_SCRATCH,
                    read_auxiliary_byte(&self.memory, entry.wrapping_add(1)),
                );
            }
            2
        } else {
            if let Some(entry) =
                find_auxiliary_type(&self.memory, target, ORDINARY_CONTACT_AUXILIARY_KIND)
            {
                self.memory.write_byte(
                    CONTACT_VALUE_SCRATCH,
                    read_auxiliary_byte(&self.memory, entry.wrapping_add(1)),
                );
            }
            1
        };

        match selector {
            0 => PathContactClass::NoObject,
            1 => PathContactClass::AuxiliaryType0b,
            _ => PathContactClass::OtherObject,
        }
    }

    fn classify_path_contact_native_outer(&mut self) -> Result<Option<PathContactClass>, Error> {
        self.suspend_coprocessor_interrupts();
        let current = self.current_object()?;
        let direct_contact = self.memory.read_byte(current + 0x20) & DIRECT_CONTACT_FLAG_20 != 0
            || self.memory.read_byte(current + 0x21) & DIRECT_CONTACT_FLAG_21 != 0;
        if direct_contact {
            let link = self
                .memory
                .read_word(current.wrapping_add(CONTACT_LINK_FIELD));
            let target = self
                .memory
                .read_word(link.wrapping_add(CONTACT_LINK_TARGET_FIELD));
            self.memory.write_word(CONTACT_TARGET_SCRATCH, target);
            let alternate = u8::from(
                self.memory.read_byte(target.wrapping_add(0x22)) & ALTERNATE_CONTACT_FLAG_22 != 0,
            );
            self.memory
                .write_byte(CONTACT_ALTERNATE_CLASS_STATE, alternate);
            return Ok(Some(self.classify_contact_target(target)));
        }

        let frame_gate = self.memory.read_byte(CONTACT_FRAME_STATE) & CONTACT_FRAME_MASK != 0;
        let above_floor = self
            .object_word(current, FIELD_Y)
            .wrapping_add(CONTACT_VERTICAL_MARGIN)
            >= 0;
        if frame_gate && above_floor {
            self.memory.write_word(CONTACT_TARGET_SCRATCH, 0);
            return Ok(Some(self.classify_contact_target(0)));
        }

        let saved_box = self
            .memory
            .read_byte(current.wrapping_add(COLLISION_BOX_EXTENSION));
        self.refresh_collision_projection()?;
        self.memory
            .write_byte(current.wrapping_add(COLLISION_BOX_EXTENSION), saved_box);
        let target = self
            .memory
            .read_word(current.wrapping_add(COLLISION_TARGET_EXTENSION));
        if target == 0
            || (self
                .memory
                .read_word(current.wrapping_add(FIELD_Y))
                .wrapping_sub(self.memory.read_word(CONTACT_TARGET_SCRATCH)) as i16)
                < 0
        {
            return Ok(None);
        }

        if self.memory.read_byte(current.wrapping_add(0x31)) & COLLISION_REACTIVE_CURRENT_FLAGS
            == COLLISION_REACTIVE_CURRENT_FLAGS
        {
            let side_flag = if self.memory.read_byte(current.wrapping_add(0x24))
                & COLLISION_CURRENT_ALTERNATE_SIDE_FLAG_24
                != 0
            {
                COLLISION_TARGET_ALTERNATE_SIDE_FLAG_26
            } else {
                COLLISION_TARGET_SIDE_FLAG_26
            };
            let flags = self.memory.read_byte(target.wrapping_add(0x26)) | side_flag;
            self.memory.write_byte(target.wrapping_add(0x26), flags);
        }
        if self.memory.read_byte(target.wrapping_add(0x26)) & COLLISION_TARGET_FLAG_26 == 0 {
            let link = self
                .memory
                .read_word(current.wrapping_add(CONTACT_LINK_FIELD));
            let fallback = self
                .memory
                .read_word(link.wrapping_add(CONTACT_LINK_TARGET_FIELD));
            self.memory.write_word(CONTACT_TARGET_SCRATCH, fallback);
            return Ok(Some(self.classify_contact_target(fallback)));
        }

        let flags = self.memory.read_byte(target.wrapping_add(0x22)) | COLLISION_TARGET_FLAG_22;
        self.memory.write_byte(target.wrapping_add(0x22), flags);
        self.memory.write_word(CONTACT_TARGET_SCRATCH, target);
        Ok(Some(self.classify_contact_target(target)))
    }

    fn ease_fixed_player_yaw(&mut self) {
        self.memory.write_word(PLAYER_YAW_DAMPING_STATE, 0);
        let yaw = self
            .memory
            .read_word(FIXED_PLAYER_OBJECT.wrapping_add(FIELD_ROT_Y));
        let half = ((yaw as i16) >> 1) as u16;
        let eighth = ((yaw as i16) >> 3) as u16;
        self.memory.write_word(PLAYER_YAW_DAMPING_SCRATCH, half);
        self.memory.write_word(
            FIXED_PLAYER_OBJECT.wrapping_add(FIELD_ROT_Y),
            half.wrapping_add(eighth),
        );
    }

    fn chase_yaw_opposite_fixed_player(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let fixed_player_yaw = self.memory.read_byte(
            FIXED_PLAYER_OBJECT
                .wrapping_add(FIELD_ROT_Y)
                .wrapping_add(1),
        );
        let target = fixed_player_yaw.wrapping_neg();
        let yaw_address = current.wrapping_add(FIELD_ROT_Y);
        let yaw = self.memory.read_byte(yaw_address);
        let step = ((target.wrapping_sub(yaw) as i8) >> 3) as u8;
        self.memory.write_byte(yaw_address, yaw.wrapping_add(step));
        Ok(())
    }

    fn configure_randomized_object_motion(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let variant = self.memory.read_byte(RANDOM_VARIANT_STATE);
        let index = usize::from(variant & RANDOMIZED_OBJECT_VARIANT_MASK);
        self.memory.write_byte(
            current.wrapping_add(OBJECT_LIFETIME_FIELD),
            RANDOMIZED_OBJECT_LIFETIME,
        );
        self.memory
            .write_byte(current.wrapping_add(OBJECT_VARIANT_FIELD), variant);

        for (address, value) in [
            (
                current.wrapping_add(OBJECT_RELATIVE_PITCH_EXTENSION),
                RANDOMIZED_OBJECT_RELATIVE_PITCH[index],
            ),
            (
                current.wrapping_add(OBJECT_MOTION_PHASE_EXTENSION),
                RANDOMIZED_OBJECT_MOTION_PHASE[index],
            ),
            (
                current.wrapping_add(OBJECT_RELATIVE_ROLL_EXTENSION),
                RANDOMIZED_OBJECT_RELATIVE_ROLL[index],
            ),
            (
                current.wrapping_add(FIELD_ROT_X),
                RANDOMIZED_OBJECT_ROTATION_X[index],
            ),
            (
                current.wrapping_add(FIELD_ROT_Y),
                RANDOMIZED_OBJECT_ROTATION_Y[index],
            ),
            (
                current.wrapping_add(FIELD_ROT_Z),
                RANDOMIZED_OBJECT_ROTATION_Z[index],
            ),
            (
                current.wrapping_add(OBJECT_AUXILIARY_BYTE_FIELD),
                RANDOMIZED_OBJECT_AUXILIARY[index],
            ),
        ] {
            self.memory.write_byte(address, value);
        }

        for (field, extension, offset) in [
            (
                FIELD_X,
                OBJECT_RELATIVE_X_EXTENSION,
                RANDOMIZED_OBJECT_POSITION_X[index],
            ),
            (
                FIELD_Y,
                OBJECT_RELATIVE_Y_EXTENSION,
                RANDOMIZED_OBJECT_POSITION_Y[index],
            ),
            (
                FIELD_Z,
                OBJECT_RELATIVE_Z_EXTENSION,
                RANDOMIZED_OBJECT_POSITION_Z[index],
            ),
        ] {
            self.memory
                .write_word(current.wrapping_add(extension), offset as u16);
            let position = self.memory.read_word(current.wrapping_add(field));
            self.memory.write_word(
                current.wrapping_add(field),
                position.wrapping_add(offset as u16),
            );
        }
        Ok(())
    }

    fn accumulate_player_auxiliary_motion(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        let direct_motion = self
            .memory
            .read_byte(PLAYER_AUXILIARY_MODE_BASE.wrapping_add(slot))
            & PLAYER_AUXILIARY_MODE_MASK
            == PLAYER_DIRECT_MOTION_MODE;
        for (current_field, alternate_player_extension) in [
            (
                OBJECT_HORIZONTAL_MOTION_FIELD,
                PLAYER_ALTERNATE_HORIZONTAL_MOTION_EXTENSION,
            ),
            (
                OBJECT_DEPTH_MOTION_FIELD,
                PLAYER_ALTERNATE_DEPTH_MOTION_EXTENSION,
            ),
        ] {
            let player_field = if direct_motion {
                current_field
            } else {
                alternate_player_extension
            };
            let value = self
                .memory
                .read_word(current.wrapping_add(current_field))
                .wrapping_add(self.memory.read_word(player.wrapping_add(player_field)));
            self.memory
                .write_word(current.wrapping_add(current_field), value);
        }
        Ok(())
    }

    fn initialize_player_auxiliary_charge(&mut self) {
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        self.memory.write_byte(
            PLAYER_AUXILIARY_CHARGE_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_CHARGE_VALUE,
        );
        let flags = self.memory.read_byte(PLAYER_AUXILIARY_CHARGE_GLOBAL_FLAGS);
        if flags & PLAYER_AUXILIARY_CHARGE_ACTIVE_BIT == 0 {
            self.memory.write_byte(
                PLAYER_AUXILIARY_CHARGE_GLOBAL_FLAGS,
                flags | PLAYER_AUXILIARY_CHARGE_ENABLE_BITS,
            );
        }
    }

    fn enable_player_auxiliary_control(&mut self) {
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        let flags_address = PLAYER_AUXILIARY_CONTROL_BASE.wrapping_add(slot);
        let flags = self.memory.read_byte(flags_address) | PLAYER_AUXILIARY_SELECTED_FLAG;
        self.memory.write_byte(flags_address, flags);
        for (base, value) in [
            (
                PLAYER_AUXILIARY_CONTROL_X_BASE,
                PLAYER_AUXILIARY_CONTROL_VALUE,
            ),
            (
                PLAYER_AUXILIARY_CONTROL_Y_BASE,
                PLAYER_AUXILIARY_CONTROL_VALUE,
            ),
            (
                PLAYER_AUXILIARY_CONTROL_Z_BASE,
                PLAYER_AUXILIARY_CONTROL_DEPTH_VALUE,
            ),
            (PLAYER_AUXILIARY_RATE_X_BASE, PLAYER_AUXILIARY_RATE_X_VALUE),
            (PLAYER_AUXILIARY_RATE_Y_BASE, PLAYER_AUXILIARY_RATE_Y_VALUE),
            (PLAYER_AUXILIARY_RATE_Z_BASE, PLAYER_AUXILIARY_RATE_Z_VALUE),
        ] {
            self.memory.write_byte(base.wrapping_add(slot), value);
        }
        if self
            .memory
            .read_byte(PLAYER_AUXILIARY_LINK_FLAGS_BASE.wrapping_add(slot))
            & PLAYER_AUXILIARY_SELECTED_FLAG
            != 0
        {
            for base in [
                PLAYER_AUXILIARY_RANGE_BASE,
                PLAYER_AUXILIARY_SECONDARY_RANGE_BASE,
            ] {
                self.memory
                    .write_word(base.wrapping_add(slot), PLAYER_AUXILIARY_RANGE_VALUE);
            }
        }
    }

    fn link_selected_object_transform(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let selected = self.memory.read_word(SELECTED_OBJECT);
        let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
        if self
            .memory
            .read_byte(PLAYER_AUXILIARY_LINK_FLAGS_BASE.wrapping_add(slot))
            & PLAYER_AUXILIARY_SELECTED_FLAG
            != 0
        {
            self.memory.write_byte(
                current.wrapping_add(OBJECT_SELECTED_LINK_FLAG_EXTENSION),
                u8::MAX,
            );
        }
        self.memory.write_word(
            current.wrapping_add(OBJECT_RELATIVE_REFERENCE_EXTENSION),
            selected,
        );
        let depth = self
            .memory
            .read_word(current.wrapping_add(OBJECT_RELATIVE_DEPTH_SOURCE_EXTENSION));
        self.memory
            .write_word(current.wrapping_add(OBJECT_RELATIVE_Z_EXTENSION), depth);
        self.memory
            .write_word(current.wrapping_add(OBJECT_RELATIVE_Y_EXTENSION), 0);
        let mode = self
            .memory
            .read_byte(PLAYER_AUXILIARY_TRANSFORM_MODE_BASE.wrapping_add(slot));
        self.memory
            .write_byte(current.wrapping_add(OBJECT_MOTION_PHASE_EXTENSION), mode);
        Ok(())
    }

    fn refresh_player_auxiliary_mode(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let pitch_address = current.wrapping_add(OBJECT_RELATIVE_PITCH_EXTENSION);
        let roll_address = current.wrapping_add(OBJECT_RELATIVE_ROLL_EXTENSION);
        self.memory.write_byte(
            pitch_address,
            self.memory
                .read_byte(pitch_address)
                .wrapping_add(PLAYER_AUXILIARY_PITCH_STEP),
        );
        self.memory.write_byte(
            roll_address,
            self.memory
                .read_byte(roll_address)
                .wrapping_add(PLAYER_AUXILIARY_ROLL_STEP),
        );

        let refresh_window = self.memory.read_byte(PLAYER_AUXILIARY_REFRESH_CLASS)
            == PLAYER_AUXILIARY_REFRESH_CLASS_VALUE
            || self.memory.read_byte(PLAYER_AUXILIARY_REFRESH_FRAME)
                & PLAYER_AUXILIARY_REFRESH_FRAME_MASK
                == 0;
        let mode_address = current.wrapping_add(OBJECT_MOTION_PHASE_EXTENSION);
        if refresh_window && self.memory.read_byte(PLAYER_AUXILIARY_REFRESH_GATE) != 0 {
            self.memory.write_byte(mode_address, 0);
            return Ok(());
        }

        let linked = self
            .memory
            .read_word(current.wrapping_add(OBJECT_LINKED_OBJECT_FIELD));
        let slot = self.memory.read_word(linked.wrapping_add(FIELD_PATH));
        let auxiliary_mode_address = PLAYER_AUXILIARY_LINK_MODE_BASE.wrapping_add(slot);
        let mut auxiliary_mode = self.memory.read_byte(auxiliary_mode_address);
        let normalization_enabled = self.memory.read_byte(PLAYER_AUXILIARY_REFRESH_OVERRIDE)
            & PLAYER_AUXILIARY_REFRESH_OVERRIDE_BIT
            != 0
            || self.memory.read_byte(PLAYER_AUXILIARY_REFRESH_DISABLE) == 0;
        if normalization_enabled && auxiliary_mode & PLAYER_AUXILIARY_MODE_ACTIVITY_MASK != 0 {
            auxiliary_mode = PLAYER_AUXILIARY_NORMALIZED_MODE;
            self.memory
                .write_byte(auxiliary_mode_address, auxiliary_mode);
        }

        let mode = auxiliary_mode & PLAYER_AUXILIARY_MODE_VALUE_MASK;
        self.memory.write_byte(mode_address, mode);
        if mode == 0 {
            self.memory.write_byte(
                auxiliary_mode_address,
                auxiliary_mode & !PLAYER_AUXILIARY_MODE_STICKY_BIT,
            );
        }
        Ok(())
    }

    fn chase_relative_word(&mut self, current: u16, extension: u16, target: i16) {
        let address = current.wrapping_add(extension);
        let value = Self::chase_word(self.memory.read_word(address) as i16, target, 3, 8);
        self.memory.write_word(address, value as u16);
        self.memory.write_word(DAMAGE_SCRATCH, value as u16);
    }

    fn chase_current_relative_offsets(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        for _ in 0..2 {
            self.chase_relative_word(
                current,
                OBJECT_RELATIVE_Z_EXTENSION,
                RELATIVE_OFFSET_CHASE_TARGET,
            );
        }
        for _ in 0..2 {
            self.chase_relative_word(current, OBJECT_RELATIVE_Y_EXTENSION, 0);
        }
        Ok(())
    }

    fn chase_current_relative_pose(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let x_target = self
            .memory
            .read_word(current.wrapping_add(OBJECT_RELATIVE_DEPTH_SOURCE_EXTENSION))
            as i16;
        self.chase_relative_word(current, OBJECT_RELATIVE_X_EXTENSION, x_target);
        for _ in 0..2 {
            self.chase_relative_word(current, OBJECT_RELATIVE_Z_EXTENSION, 0);
        }
        self.chase_relative_word(current, OBJECT_RELATIVE_Y_EXTENSION, 0);
        Ok(())
    }

    fn advance_current_relative_offsets(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let pitch_address = current.wrapping_add(OBJECT_RELATIVE_PITCH_EXTENSION);
        self.memory.write_byte(
            pitch_address,
            self.memory
                .read_byte(pitch_address)
                .wrapping_sub(RELATIVE_OFFSET_ANGLE_STEP),
        );
        let y_address = current.wrapping_add(OBJECT_RELATIVE_Y_EXTENSION);
        self.memory.write_word(
            y_address,
            self.memory
                .read_word(y_address)
                .wrapping_add(RELATIVE_OFFSET_POSITION_STEP),
        );
        let z_address = current.wrapping_add(OBJECT_RELATIVE_Z_EXTENSION);
        self.memory.write_word(
            z_address,
            self.memory
                .read_word(z_address)
                .wrapping_sub(RELATIVE_OFFSET_POSITION_STEP),
        );
        let phase = self
            .memory
            .read_byte(current.wrapping_add(OBJECT_MOTION_PHASE_EXTENSION));
        for extension in [
            OBJECT_RELATIVE_ROLL_EXTENSION,
            OBJECT_RELATIVE_YAW_EXTENSION,
        ] {
            let address = current.wrapping_add(extension);
            self.memory
                .write_byte(address, self.memory.read_byte(address).wrapping_add(phase));
        }
        Ok(())
    }

    fn update_conditional_object_phase(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let phase_address = current.wrapping_add(OBJECT_MOTION_PHASE_EXTENSION);
        let delay_address = current.wrapping_add(OBJECT_CONDITIONAL_PHASE_EXTENSION);
        self.memory.write_byte(phase_address, 0);
        self.memory.write_word(delay_address, 0);
        if self.memory.read_byte(CONDITIONAL_OBJECT_PHASE_STATE) < CONDITIONAL_OBJECT_PHASE_LIMIT {
            self.memory
                .write_byte(phase_address, CONDITIONAL_OBJECT_ACTIVE_PHASE);
            if self.memory.read_byte(CONDITIONAL_OBJECT_CONTROL_FLAGS)
                & CONDITIONAL_OBJECT_CONTROL_MASK
                == 0
            {
                self.memory
                    .write_word(delay_address, CONDITIONAL_OBJECT_DELAY);
            }
        }
        Ok(())
    }

    fn initialize_player_relative_motion(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        let linked_transform = self
            .memory
            .read_byte(PLAYER_AUXILIARY_LINK_FLAGS_BASE.wrapping_add(slot))
            & PLAYER_AUXILIARY_SELECTED_FLAG
            != 0;
        let offset = if linked_transform {
            let pitch = self.memory.read_byte(player.wrapping_add(FIELD_ROT_X));
            let yaw = self.memory.read_byte(player.wrapping_add(FIELD_ROT_Y));
            let (y, depth) =
                sf_core::snes_trig::rotate_16yz(pitch, 0, PLAYER_RELATIVE_DEPTH_OFFSET);
            let (x, z) = sf_core::snes_trig::rotate_16xz(yaw, 0, depth);
            [x, y, z]
        } else {
            [0, 0, 0]
        };
        let player_position = [
            self.memory.read_word(player.wrapping_add(FIELD_X)),
            self.memory.read_word(player.wrapping_add(FIELD_Y)),
            self.memory.read_word(player.wrapping_add(FIELD_Z)),
        ];
        for ((field, player_value), offset) in [FIELD_X, FIELD_Y, FIELD_Z]
            .into_iter()
            .zip(player_position)
            .zip(offset)
        {
            self.memory.write_word(
                current.wrapping_add(field),
                player_value.wrapping_add(offset as u16),
            );
        }
        Ok(())
    }

    fn initialize_spawned_object_motion(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let spawned = self.memory.read_word(LAST_SPAWNED_OBJECT);
        let current_yaw = self.memory.read_byte(current.wrapping_add(FIELD_ROT_Y));
        let (initial_x, initial_z) =
            sf_core::snes_trig::rotate_16xz(current_yaw, 0, SPAWNED_OBJECT_DEPTH_OFFSET);
        for (field, offset) in [
            (FIELD_X, initial_x.wrapping_shl(SPAWNED_OBJECT_OFFSET_SCALE)),
            (
                FIELD_Y,
                SPAWNED_OBJECT_VERTICAL_OFFSET.wrapping_shl(SPAWNED_OBJECT_OFFSET_SCALE),
            ),
            (FIELD_Z, initial_z.wrapping_shl(SPAWNED_OBJECT_OFFSET_SCALE)),
        ] {
            let position = self.memory.read_word(current.wrapping_add(field));
            self.memory.write_word(
                spawned.wrapping_add(field),
                position.wrapping_add(offset as u16),
            );
        }

        let yaw = self.memory.read_byte(SPAWNED_OBJECT_YAW_STATE);
        self.memory.write_byte(spawned.wrapping_add(FIELD_ROT_X), 0);
        self.memory
            .write_byte(spawned.wrapping_add(FIELD_ROT_Y), yaw);
        self.memory.write_byte(spawned.wrapping_add(FIELD_ROT_Z), 0);
        let variant = usize::from(
            self.memory.read_byte(RANDOM_VARIANT_STATE) & RANDOMIZED_OBJECT_VARIANT_MASK,
        );
        let (variant_x, variant_z) =
            sf_core::snes_trig::rotate_16xz(yaw, 0, SPAWNED_OBJECT_VARIANT_DEPTH[variant]);
        for (field, offset) in [
            (FIELD_X, variant_x.wrapping_shl(SPAWNED_OBJECT_OFFSET_SCALE)),
            (FIELD_Z, variant_z.wrapping_shl(SPAWNED_OBJECT_OFFSET_SCALE)),
        ] {
            let position = self.memory.read_word(spawned.wrapping_add(field));
            self.memory.write_word(
                spawned.wrapping_add(field),
                position.wrapping_add(offset as u16),
            );
        }
        Ok(())
    }

    fn apply_current_health_decay(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let snapshot = self
            .memory
            .read_byte(current.wrapping_add(OBJECT_HEALTH_SNAPSHOT_FIELD));
        let hit_points = self
            .memory
            .read_byte(current.wrapping_add(OBJECT_HIT_POINTS_FIELD));
        let observed_damage = snapshot.wrapping_sub(hit_points);
        self.memory.write_byte(DAMAGE_SCRATCH, observed_damage);
        self.memory
            .write_byte(current.wrapping_add(OBJECT_HIT_POINTS_FIELD), snapshot);

        // Retail branches on the sign flag after an eight-bit comparison,
        // rather than applying an ordinary unsigned clamp.
        let decay = if (observed_damage.wrapping_sub(DAMAGE_CLAMP_THRESHOLD) as i8) >= 0 {
            self.memory.write_byte(DAMAGE_SCRATCH, DAMAGE_CLAMP_AMOUNT);
            DAMAGE_CLAMP_AMOUNT
        } else {
            observed_damage
        };
        let reduced = snapshot.wrapping_sub(decay);
        let reduced = if (reduced as i8) >= 0 { reduced } else { 0 };
        for address in [
            current.wrapping_add(OBJECT_HIT_POINTS_FIELD),
            current.wrapping_add(OBJECT_HEALTH_SNAPSHOT_FIELD),
            DAMAGE_GLOBAL_MIRROR,
        ] {
            self.memory.write_byte(address, reduced);
        }
        Ok(())
    }

    fn separate_yaw_targets(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let relative_address = current.wrapping_add(OBJECT_RELATIVE_YAW_EXTENSION);
        let relative = self.memory.read_byte(relative_address);
        let yaw_address = current.wrapping_add(FIELD_ROT_Y);
        let yaw = self.memory.read_byte(yaw_address);
        if (relative.wrapping_sub(yaw) as i8) >= 0 {
            self.memory
                .write_byte(yaw_address, yaw.wrapping_sub(YAW_SEPARATION_STEP));
            self.memory
                .write_byte(relative_address, relative.wrapping_add(YAW_SEPARATION_STEP));
        } else {
            self.memory
                .write_byte(yaw_address, yaw.wrapping_add(YAW_SEPARATION_STEP));
            self.memory
                .write_byte(relative_address, relative.wrapping_sub(YAW_SEPARATION_STEP));
        }
        Ok(())
    }

    fn advance_vertical_oscillation(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let position_address = current.wrapping_add(FIELD_Y);
        let position = self.memory.read_word(position_address);
        let centered = position.wrapping_add(VERTICAL_OSCILLATION_CENTER as u16);
        let should_move_positive = if (centered as i16) < 0 {
            true
        } else if (centered.wrapping_sub(VERTICAL_OSCILLATION_SPAN) as i16) >= 0 {
            false
        } else {
            let player = self.memory.read_word(PLAYER_ONE);
            let player_y = self.memory.read_word(player.wrapping_add(FIELD_Y));
            (player_y.wrapping_sub(position) as i16) >= 0
        };

        let velocity = self
            .memory
            .read_word(current.wrapping_add(OBJECT_VERTICAL_VELOCITY_FIELD));
        let velocity = match (should_move_positive, (velocity as i16) >= 0) {
            (true, false) | (false, true) => velocity.wrapping_neg(),
            _ => velocity,
        };
        self.memory.write_word(
            position_address,
            position.wrapping_add(velocity.wrapping_mul(2)),
        );
        Ok(())
    }

    fn current_object(&self) -> Result<u16, Error> {
        let object = self.memory.read_word(CURRENT_OBJECT);
        object_index(object)
            .map(|_| object)
            .ok_or(Error::InvalidObject(object))
    }

    fn selected_object(&self) -> Option<u16> {
        let object = self.memory.read_word(SELECTED_OBJECT);
        object_index(object).map(|_| object)
    }

    fn variable_address(&self, id: u8) -> Result<u16, Error> {
        let object = self.current_object()?;
        Ok(if id & 0x80 == 0 {
            object.wrapping_add(u16::from(id))
        } else {
            object.wrapping_add(0x1C41).wrapping_add(u16::from(id))
        })
    }

    fn object_word(&self, object: u16, field: u16) -> i16 {
        self.memory.read_word(object.wrapping_add(field)) as i16
    }

    fn set_object_word(&mut self, object: u16, field: u16, value: i16) {
        self.memory
            .write_word(object.wrapping_add(field), value as u16);
    }

    fn object_delta(&self, from: u16, to: u16) -> (i16, i16, i16) {
        (
            self.object_word(to, FIELD_X)
                .wrapping_sub(self.object_word(from, FIELD_X)),
            self.object_word(to, FIELD_Y)
                .wrapping_sub(self.object_word(from, FIELD_Y)),
            self.object_word(to, FIELD_Z)
                .wrapping_sub(self.object_word(from, FIELD_Z)),
        )
    }

    fn face_object(&mut self, target: u16, smooth_shift: Option<u32>) -> Result<(), Error> {
        let current = self.current_object()?;
        let (dx, dy, dz) = self.object_delta(current, target);
        let target_yaw = sf_core::aim_angle::yanglexy(dx, dz).wrapping_neg();
        let target_pitch = sf_core::aim_angle::atan2_to_u8(
            f32::from(dy),
            f32::from(sf2_xz_angle_distance(dx, dz)),
        );
        for (field, target) in [(FIELD_ROT_X, target_pitch), (FIELD_ROT_Y, target_yaw)] {
            let address = current + field;
            let mut value = self.memory.read_byte(address);
            if let Some(shift) = smooth_shift {
                sf_core::snes_trig::achase_angle_8(&mut value, target, shift);
            } else {
                value = target;
            }
            self.memory.write_byte(address, value);
        }
        Ok(())
    }

    fn selected_aux_address(&self, base: u16) -> Result<u16, Error> {
        let selected = self.selected_object().ok_or(Error::InvalidObject(0))?;
        Ok(base.wrapping_add(self.memory.read_word(selected + FIELD_PATH)))
    }

    fn spawn_full(&mut self, spawn: ObjectSpawn) -> Result<u16, Error> {
        let current = self.current_object()?;
        let object = allocate(&mut self.memory, current).ok_or(Error::ObjectPoolExhausted)?;
        self.memory.write_word(object + FIELD_SHAPE, spawn.shape);
        self.memory
            .write_word(object + FIELD_PATH, spawn.path.offset);
        self.memory.write_word(object + FIELD_STRATEGY, 0x7E1E);
        self.memory.write_byte(object + FIELD_STRATEGY + 2, 0x7F);
        for (field, value) in [
            (FIELD_ROT_X, spawn.rotation[0]),
            (FIELD_ROT_Y, spawn.rotation[1]),
            (FIELD_ROT_Z, spawn.rotation[2]),
        ] {
            self.memory.write_byte(object + field, value);
        }
        // `$7F:9042` and `$7F:91A3` both store the two one-byte combat
        // operands in the canonical object fields `$2D/$2E`.
        self.memory.write_byte(object + 0x2D, spawn.hit_points);
        self.memory.write_byte(object + 0x2E, spawn.attack_points);
        for (field, offset) in [
            (FIELD_X, spawn.offset[0]),
            (FIELD_Y, spawn.offset[1]),
            (FIELD_Z, spawn.offset[2]),
        ] {
            let origin = self.object_word(current, field);
            self.set_object_word(object, field, origin.wrapping_add(offset));
        }
        self.memory.write_byte(object + 0x20, 0x08);
        let class = self.memory.read_byte(object + 0x31) | 0x10;
        self.memory.write_byte(object + 0x31, class);
        if self.memory.read_byte(current + 0x24) & 0x80 != 0 {
            let flags = self.memory.read_byte(object + 0x24) | 0x80;
            self.memory.write_byte(object + 0x24, flags);
        }
        self.memory.write_byte(
            object.wrapping_add(0x1CF0),
            self.memory.read_byte(current.wrapping_add(0x1CF0)),
        );
        // Retail path spawners return with X still naming the source object
        // and publish the new object through `$D771` for BECOME. `allocate`
        // updates the host's current-object mirror, so restore it explicitly.
        self.memory.write_word(0xD771, object);
        self.memory.write_word(CURRENT_OBJECT, current);
        Ok(object)
    }

    fn find_child(&self, child_number: u8) -> Result<u16, Error> {
        let current = self.current_object()?;
        let root = if self.memory.read_byte(current + 0x23) & 0x10 != 0 {
            current
        } else {
            self.memory.read_word(current + 0x06)
        };
        if root == 0 {
            return Ok(0);
        }
        let mut child = self.memory.read_word(root + 0x29);
        while child != 0 {
            if self.memory.read_byte(child + 0x13) == child_number {
                return Ok(child);
            }
            child = self.memory.read_word(child + 0x29);
        }
        Ok(0)
    }

    fn refresh_relative_yaw(&mut self, current: u16) {
        if self.memory.read_byte(current + 0x25) & 0x04 == 0 {
            return;
        }
        let reference = self.memory.read_word(current.wrapping_add(0x1CD8));
        if reference == 0 {
            return;
        }
        let delta = self
            .memory
            .read_byte(current + FIELD_ROT_Y)
            .wrapping_sub(self.memory.read_byte(reference + FIELD_ROT_Y));
        self.memory.write_byte(current.wrapping_add(0x1CD6), delta);
    }

    fn enqueue_event(&mut self, event: u16, target: u16) {
        let index = u16::from(self.memory.read_byte(0x1D16) & 0x1F);
        let event = if target == self.memory.read_word(PLAYER_ONE) || target == 0x033F {
            event
        } else {
            event | 0x8000
        };
        self.memory.write_word(0x1CF6u16.wrapping_add(index), event);
        self.memory
            .write_byte(0x1D16, (index as u8).wrapping_add(2) & 0x1F);
    }

    fn enqueue_player_event(&mut self, event: u8) {
        self.enqueue_event(u16::from(event), self.memory.read_word(PLAYER_ONE));
    }

    fn configure_pilot_auxiliary(
        &mut self,
        value: u16,
        mode: PilotAuxiliaryMode,
    ) -> Result<(), Error> {
        let current = self.current_object()?;
        let player = self.memory.read_word(PLAYER_ONE);
        if object_index(player).is_none() {
            return Ok(());
        }
        let slot = self.memory.read_word(player + FIELD_PATH);
        let flags_address = 0x6A8Cu16.wrapping_add(slot);
        let flags = self.memory.read_byte(flags_address) & !0x40;
        self.memory.write_byte(flags_address, flags);
        if flags & 0x80 != 0 {
            return Ok(());
        }

        let value = if matches!(mode, PilotAuxiliaryMode::DoubledValue) {
            (value & 0xFF00) | u16::from((value as u8).wrapping_shl(1))
        } else {
            value
        };
        self.memory.write_word(0x6C1C + slot, 2);
        for (source, target) in [
            (FIELD_X, 0x6A92u16),
            (FIELD_Y, 0x6A94u16),
            (FIELD_Z, 0x6A96u16),
        ] {
            self.memory.write_word(
                target.wrapping_add(slot),
                self.memory.read_word(current + source),
            );
        }
        self.memory.write_word(0x6C24 + slot, 0x00FF);
        self.memory.write_word(0x6A90 + slot, value);
        self.memory
            .write_word(0x6C26 + slot, if (value as i16) < 0 { 1 } else { value });
        self.memory.write_word(0x6A98 + slot, current);
        self.memory.write_byte(0x6C29 + slot, 3);
        self.memory.write_byte(0x6C28 + slot, 0x1F);

        let axis_modes = match mode {
            PilotAuxiliaryMode::DoubledValue => [1, 2, 2],
            PilotAuxiliaryMode::AlternateAxes => [6, 6, 3],
            PilotAuxiliaryMode::FullControl => [4, 8, 8],
        };
        for (address, byte) in [0x6A8Du16, 0x6A8E, 0x6A8F].into_iter().zip(axis_modes) {
            self.memory.write_byte(address + slot, byte);
        }
        for address in [0x6C2Au16, 0x6C2B, 0x6C2C] {
            self.memory.write_byte(address + slot, 0x1F);
        }
        // `$07:B833` refreshes the stored origin when this object owns the
        // slot; B79F just installed that ownership above.
        for (source, target) in [
            (FIELD_X, 0x6A92u16),
            (FIELD_Y, 0x6A94u16),
            (FIELD_Z, 0x6A96u16),
        ] {
            self.memory.write_word(
                target.wrapping_add(slot),
                self.memory.read_word(current + source),
            );
        }
        Ok(())
    }

    fn set_player_aux_mode(&mut self, enabled: bool) -> Result<(), Error> {
        let current = self.current_object()?;
        let global = self.memory.read_word(0x1B84);
        let flags = self.memory.read_byte(current + 0x26);
        if enabled {
            self.memory.write_word(0x1B84, global | 0x0002);
            self.memory.write_byte(current + 0x26, flags | 0x08);

            // `$03:A6A5` marks every active object in the two retail classes
            // invalid before the player record is moved into auxiliary RAM.
            for object in active_objects(&self.memory) {
                let class = self.memory.read_byte(object + 0x31);
                if class & 0x50 == 0x50 || class & 0x08 != 0 {
                    let value = self.memory.read_byte(object + 0x26) | 0x08;
                    self.memory.write_byte(object + 0x26, value);
                    let value = self.memory.read_byte(object + 0x21) | 0x01;
                    self.memory.write_byte(object + 0x21, value);
                    self.memory.write_byte(object + 0x2D, 0);
                }
            }

            let block = allocate_auxiliary(&mut self.memory, current, 0x003F)
                .ok_or(Error::AuxiliaryHeapExhausted)?;
            for offset in 0..0x003Fu16 {
                let value = self.memory.read_byte(0x033F + offset);
                write_auxiliary_byte(&mut self.memory, block + offset, value);
            }
            let entry = get_or_create_auxiliary_type(&mut self.memory, current, 0x08)
                .ok_or(Error::AuxiliaryHeapExhausted)?;
            write_auxiliary_word(&mut self.memory, entry + 1, block);
            self.enqueue_player_event(0xF8);
        } else {
            self.memory.write_word(0x1B84, global & !0x0002);
            self.memory.write_byte(current + 0x26, flags & !0x08);
            if let Some(entry) = find_auxiliary_type(&self.memory, current, 0x08) {
                let block = read_auxiliary_word(&self.memory, entry + 1);
                for offset in 0..0x003Fu16 {
                    let value = read_auxiliary_byte(&self.memory, block + offset);
                    self.memory.write_byte(0x033F + offset, value);
                }
                free_auxiliary(&mut self.memory, current, block);
            }
            self.enqueue_player_event(0xF7);
        }
        Ok(())
    }

    /// SF2 `$7F:306E` / `$7F:2D1F`: generate the three signed velocity
    /// words from an object's byte yaw, pitch and speed.  This is the same
    /// signed logarithmic multiply sequence used by the original Star Fox
    /// engine, including the negated yaw and truncation after each product.
    fn regenerate_object_velocity(&mut self, object: u16) {
        let yaw = self.memory.read_byte(object + FIELD_ROT_Y).wrapping_neg();
        let pitch = self.memory.read_byte(object + FIELD_ROT_X);
        let speed = self.memory.read_byte(object + 0x18) as i8;
        let cos_pitch = sf_core::snes_trig::COSTAB[pitch as usize];
        let vx = sf_core::snes_trig::mulslog_mac8(
            sf_core::snes_trig::mulslog_mac8(speed, sf_core::snes_trig::SINTAB[yaw as usize]),
            cos_pitch,
        ) as i16;
        let vy = sf_core::snes_trig::mulslog_mac8(speed, sf_core::snes_trig::SINTAB[pitch as usize])
            as i16;
        let vz = sf_core::snes_trig::mulslog_mac8(
            sf_core::snes_trig::mulslog_mac8(speed, sf_core::snes_trig::COSTAB[yaw as usize]),
            cos_pitch,
        ) as i16;
        for (field, value) in [(0x32, vx), (0x34, vy), (0x36, vz)] {
            self.set_object_word(object, field, value);
        }
    }

    fn initialize_launched_external_object(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let external = self.memory.read_word(LAUNCHED_EXTERNAL_OBJECT);

        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            let value = self.memory.read_word(current.wrapping_add(field));
            self.memory.write_word(external.wrapping_add(field), value);
        }
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            let value = self.memory.read_byte(current.wrapping_add(field));
            self.memory.write_byte(external.wrapping_add(field), value);
        }

        self.memory.write_byte(
            external.wrapping_add(OBJECT_LIFETIME_FIELD),
            LAUNCHED_EXTERNAL_SPEED,
        );
        self.regenerate_object_velocity(external);

        let mut remaining = LAUNCHED_EXTERNAL_STEP_LIMIT;
        self.memory
            .write_byte(LAUNCHED_EXTERNAL_STEP_COUNTER, remaining);
        let cleared_launch_boundary = loop {
            remaining = remaining.wrapping_sub(1);
            self.memory
                .write_byte(LAUNCHED_EXTERNAL_STEP_COUNTER, remaining);
            if remaining == 0 {
                break false;
            }

            for (position, velocity) in [
                (FIELD_X, OBJECT_HORIZONTAL_MOTION_FIELD),
                (FIELD_Y, OBJECT_VERTICAL_VELOCITY_FIELD),
            ] {
                let value = self
                    .memory
                    .read_word(external.wrapping_add(position))
                    .wrapping_add(self.memory.read_word(external.wrapping_add(velocity)));
                self.memory
                    .write_word(external.wrapping_add(position), value);
            }

            let (depth, depth_carry) = self
                .memory
                .read_word(external.wrapping_add(FIELD_Z))
                .overflowing_add(
                    self.memory
                        .read_word(external.wrapping_add(OBJECT_DEPTH_MOTION_FIELD)),
                );
            self.memory
                .write_word(external.wrapping_add(FIELD_Z), depth);

            let vertical = self.memory.read_word(external.wrapping_add(FIELD_Y));
            let boundary = vertical
                .wrapping_add(LAUNCHED_EXTERNAL_CLEARANCE)
                .wrapping_add(u16::from(depth_carry));
            if boundary as i16 >= 0 {
                break true;
            }
        };

        if cleared_launch_boundary {
            self.memory.write_word(
                LAUNCHED_EXTERNAL_POSITION_X,
                self.memory.read_word(external.wrapping_add(FIELD_X)),
            );
            self.memory.write_word(
                LAUNCHED_EXTERNAL_POSITION_Z,
                self.memory.read_word(external.wrapping_add(FIELD_Z)),
            );
        }
        Ok(())
    }

    fn spawn_player_linked_object(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        let mode = self
            .memory
            .read_byte(PLAYER_AUXILIARY_LINK_MODE_BASE.wrapping_add(slot));
        if mode & PLAYER_LINKED_OBJECT_ACTIVE_MODE_MASK == 0 {
            return Ok(());
        }

        let Some(object) = allocate(&mut self.memory, current) else {
            return Ok(());
        };
        self.memory.write_word(CURRENT_OBJECT, current);
        self.memory
            .write_word(object.wrapping_add(FIELD_SHAPE), PLAYER_LINKED_OBJECT_SHAPE);
        self.memory
            .write_word(object.wrapping_add(OBJECT_LINKED_OBJECT_FIELD), current);
        self.memory.write_byte(
            object.wrapping_add(PLAYER_LINKED_OBJECT_PARAMETER_FIELD),
            PLAYER_LINKED_OBJECT_PARAMETER,
        );

        self.memory.write_word(
            object.wrapping_add(FIELD_STRATEGY),
            PLAYER_LINKED_OBJECT_STRATEGY,
        );
        self.memory.write_byte(
            object.wrapping_add(FIELD_STRATEGY + 2),
            PLAYER_LINKED_OBJECT_STRATEGY_BANK,
        );
        self.memory.write_byte(
            object.wrapping_add(PLAYER_LINKED_OBJECT_EXTENSION_STATE),
            u8::MAX,
        );
        self.memory.write_byte(
            object.wrapping_add(OBJECT_HIT_POINTS_FIELD),
            PLAYER_LINKED_OBJECT_HIT_POINTS,
        );
        self.memory.write_byte(
            object.wrapping_add(OBJECT_VARIANT_FIELD),
            PLAYER_LINKED_OBJECT_ATTACK_POINTS,
        );

        for (field, mask) in [
            (0x21, PLAYER_LINKED_OBJECT_FLAG_21),
            (0x22, PLAYER_LINKED_OBJECT_FLAG_22),
            (0x23, PLAYER_LINKED_OBJECT_FLAG_23),
            (OBJECT_FLAG_25_FIELD, PLAYER_LINKED_OBJECT_FLAG_25),
            (0x26, PLAYER_LINKED_OBJECT_FLAG_26),
        ] {
            let flags = self.memory.read_byte(object.wrapping_add(field)) | mask;
            self.memory.write_byte(object.wrapping_add(field), flags);
        }
        let class =
            self.memory.read_byte(object.wrapping_add(0x31)) & PLAYER_LINKED_OBJECT_CLASS_MASK;
        self.memory.write_byte(object.wrapping_add(0x31), class);

        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            let value = self.memory.read_word(current.wrapping_add(field));
            self.memory.write_word(object.wrapping_add(field), value);
        }
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            let value = self.memory.read_byte(current.wrapping_add(field));
            self.memory.write_byte(object.wrapping_add(field), value);
        }

        self.memory
            .write_byte(PLAYER_LINKED_OBJECT_ACTIVITY_STATE, 1);
        Ok(())
    }

    fn reset_player_auxiliary_target(&mut self) -> Result<(), Error> {
        let current = self.current_object()?;
        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));

        let flags_address = PLAYER_AUXILIARY_CONTROL_BASE.wrapping_add(slot);
        let flags = self.memory.read_byte(flags_address)
            & !(PLAYER_AUXILIARY_SELECTED_FLAG | PLAYER_AUXILIARY_CONFIGURATION_LOCK);
        self.memory.write_byte(flags_address, flags);
        self.memory.write_byte(
            PLAYER_AUXILIARY_TARGET_DELAY_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_TARGET_DELAY,
        );

        self.memory.write_word(
            PLAYER_AUXILIARY_TARGET_MODE_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_TARGET_MODE,
        );
        for (source, target) in [(FIELD_X, 0x6A92u16), (FIELD_Y, 0x6A94), (FIELD_Z, 0x6A96)] {
            let value = self.memory.read_word(current.wrapping_add(source));
            self.memory.write_word(target.wrapping_add(slot), value);
        }
        self.memory.write_word(
            PLAYER_AUXILIARY_TARGET_LIMIT_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_TARGET_LIMIT,
        );
        self.memory
            .write_word(PLAYER_AUXILIARY_RANGE_BASE.wrapping_add(slot), 0);
        self.memory.write_word(
            PLAYER_AUXILIARY_TARGET_SECONDARY_LIMIT_BASE.wrapping_add(slot),
            0,
        );
        self.memory.write_word(
            PLAYER_AUXILIARY_TARGET_OWNER_BASE.wrapping_add(slot),
            current,
        );
        self.memory.write_byte(
            PLAYER_AUXILIARY_TARGET_AXIS_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_TARGET_AXIS,
        );
        self.memory.write_byte(
            PLAYER_AUXILIARY_TARGET_CONTROL_BASE.wrapping_add(slot),
            PLAYER_AUXILIARY_TARGET_CONTROL,
        );

        for (base, value) in [
            (PLAYER_AUXILIARY_RATE_X_BASE, PLAYER_AUXILIARY_TARGET_RATE_X),
            (PLAYER_AUXILIARY_RATE_Y_BASE, PLAYER_AUXILIARY_TARGET_RATE_Y),
            (PLAYER_AUXILIARY_RATE_Z_BASE, PLAYER_AUXILIARY_TARGET_RATE_Z),
            (
                PLAYER_AUXILIARY_CONTROL_X_BASE,
                PLAYER_AUXILIARY_TARGET_RANGE_X,
            ),
            (
                PLAYER_AUXILIARY_CONTROL_Y_BASE,
                PLAYER_AUXILIARY_TARGET_RANGE_Y,
            ),
            (
                PLAYER_AUXILIARY_CONTROL_Z_BASE,
                PLAYER_AUXILIARY_TARGET_RANGE_Z,
            ),
        ] {
            self.memory.write_byte(base.wrapping_add(slot), value);
        }

        self.memory
            .write_byte(flags_address, flags | PLAYER_AUXILIARY_SELECTED_FLAG);
        Ok(())
    }

    /// Shared weapon formatter reached through `$03:A89C` by the five
    /// selectors installed by opcode `$039` and selector `$02` used by the
    /// linked-object effect service.
    fn spawn_path_weapon(&mut self, source: u16, selector: u8) -> Option<u16> {
        let source_is_player = source == self.memory.read_word(PLAYER_ONE)
            || source == self.memory.read_word(PLAYER_TWO);
        let path = match selector {
            0x02 if source_is_player => 0xF029,
            0x02 => 0xEEED,
            0x12 => 0xEF2D,
            0x14 => 0xEE4C,
            0x16 => 0xEE3B,
            0x1A => 0xEC98,
            0x1E => 0xECF7,
            _ => return None,
        };
        // `$0D:E017` temporarily makes the source the active-list head while
        // `l_add` runs, then restores the real head.  A zero source therefore
        // produces the same intentionally orphaned fallback record as retail.
        let active_head = self.memory.read_word(ACTIVE_LIST);
        self.memory.write_word(ACTIVE_LIST, source);
        let object = match allocate(&mut self.memory, source) {
            Some(object) => object,
            None => {
                self.memory.write_word(ACTIVE_LIST, active_head);
                return None;
            }
        };
        self.memory.write_word(ACTIVE_LIST, active_head);

        // `$0D:E017` followed by `$03:AB1D`, with the six launch offsets
        // cleared by `$7F:88C4`.
        self.memory.write_word(object + FIELD_SHAPE, 0xBC9C);
        self.memory.write_word(object + FIELD_STRATEGY, 0x7E1E);
        self.memory.write_byte(object + FIELD_STRATEGY + 2, 0x7F);
        self.memory.write_byte(object + 0x09, 2);
        self.memory.write_byte(object + 0x2D, 1);
        self.memory.write_byte(object + 0x2E, 1);
        for field in [FIELD_X, FIELD_Y, FIELD_Z] {
            let value = self.memory.read_word(source + field);
            self.memory.write_word(object + field, value);
        }
        for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
            let value = self.memory.read_byte(source + field);
            self.memory.write_byte(object + field, value);
        }
        self.memory.write_byte(object + FIELD_ROT_Z, 0);
        self.memory.write_byte(
            object + FIELD_ROT_X,
            self.memory
                .read_byte(object + FIELD_ROT_X)
                .wrapping_add(self.memory.read_byte(0x14B7)),
        );
        self.memory.write_byte(
            object + FIELD_ROT_Y,
            self.memory
                .read_byte(object + FIELD_ROT_Y)
                .wrapping_add(self.memory.read_byte(0x14B6)),
        );
        self.memory
            .write_byte(object + 0x13, self.memory.read_byte(object + FIELD_ROT_X));
        self.memory
            .write_byte(object + 0x15, self.memory.read_byte(object + FIELD_ROT_Y));
        self.memory
            .write_byte(object + 0x17, self.memory.read_byte(source + 0x18));
        self.memory.write_word(object + 0x1C, source);
        self.memory.write_word(source + 0x1C, object);
        for (field, mask) in [(0x22, 0x04), (0x24, 0x04), (0x25, 0x02), (0x31, 0x02)] {
            let value = self.memory.read_byte(object + field) | mask;
            self.memory.write_byte(object + field, value);
        }
        self.memory.write_byte(object.wrapping_add(0x1CCB), 0x80);
        let flags = self.memory.read_byte(object + 0x26) | 0x10;
        self.memory.write_byte(object + 0x26, flags);
        self.memory.write_word(object + FIELD_PATH, path);

        match selector {
            0x02 => {
                self.memory.write_byte(object + 0x2D, 0x78);
                self.memory.write_byte(object + 0x2E, 0x02);
            }
            0x12 => {
                let flags = self.memory.read_byte(object + 0x25) & !0x02;
                self.memory.write_byte(object + 0x25, flags);
            }
            0x14 | 0x1A => {
                let player = self.memory.read_word(PLAYER_ONE);
                let alternate_speed = object_index(player)
                    .map(|_| {
                        let slot = self.memory.read_word(player + FIELD_PATH);
                        self.memory.read_byte(0x6AA0u16.wrapping_add(slot)) & 0xF0 == 0x10
                    })
                    .unwrap_or(false);
                let speed = match (selector, alternate_speed) {
                    (0x14, true) | (0x1A, true) => 0x28,
                    (0x14, false) => 0x46,
                    (0x1A, false) => 0x3C,
                    _ => unreachable!(),
                };
                self.memory.write_byte(object + 0x18, speed);
                self.regenerate_object_velocity(object);
            }
            _ => {}
        }

        // `$0D:DE38` classifies every used weapon except selector `$12`.
        if selector != 0x12 && (selector != 0x02 || !source_is_player) {
            let player = self.memory.read_word(PLAYER_ONE);
            let player_yaw = if object_index(player).is_some() {
                self.memory.read_byte(player + FIELD_ROT_Y)
            } else {
                0
            };
            let difference = player_yaw
                .wrapping_add(0x80)
                .wrapping_sub(self.memory.read_byte(object + FIELD_ROT_Y))
                .wrapping_add(0x40);
            if difference >= 0x80 {
                if self.random_byte() & 3 != 0 {
                    let flags = self.memory.read_byte(object + 0x21) | 1;
                    self.memory.write_byte(object + 0x21, flags);
                    let count = self.memory.read_byte(0x1D69).wrapping_add(1);
                    self.memory.write_byte(0x1D69, count);
                }
                let count = self.memory.read_byte(0x1D6B).wrapping_add(1);
                self.memory.write_byte(0x1D6B, count);
            }
            let class = self.memory.read_byte(object + 0x31) | 0x50;
            self.memory.write_byte(object + 0x31, class);
        }
        Some(object)
    }

    fn chase_word(current: i16, target: i16, shift: u32, minimum: i16) -> i16 {
        if current == target {
            return current;
        }
        let difference = target.wrapping_sub(current);
        let limited = if difference > 0 && difference < minimum {
            minimum
        } else if difference < 0 && difference > minimum.wrapping_neg() {
            minimum.wrapping_neg()
        } else {
            difference
        };
        current.wrapping_add(limited / (1i16 << shift))
    }
}

use sf_core::aim_angle::sf2_xz_angle_distance;

#[inline]
fn sf2_signed_half(value: u16) -> u16 {
    ((value as i16) >> 1) as u16
}

#[inline]
fn sf2_abs_word(value: u16) -> u16 {
    if value as i16 >= 0 {
        value
    } else {
        value.wrapping_neg()
    }
}

impl Game {
    /// SF2's CPU-side `$7F:1D7E` arctangent.  The routine reduces the operands
    /// to a Q14 ratio and indexes the retail 512-word arctangent table.  Keeping
    /// the ROM table here (rather than a generated float table) also preserves
    /// the original quantization at octant boundaries.
    fn sf2_atan16(&self, x: u16, y: u16) -> u16 {
        let original_x = x;
        let original_y = y;
        let mut angle = if y == 0 {
            0x4000
        } else {
            let mut numerator = sf2_abs_word(x);
            let mut denominator = sf2_abs_word(y);
            if numerator == denominator {
                0x2000
            } else {
                // The retail CMP/BMI swaps when the subtraction is nonnegative.
                // Coordinates reaching this helper have already been normalized
                // below $1000, so this is the ordinary unsigned greater-than case.
                let swapped = numerator.wrapping_sub(denominator) as i16 >= 0;
                if swapped {
                    std::mem::swap(&mut numerator, &mut denominator);
                }
                let ratio = if denominator == 0 {
                    0x7FFF
                } else {
                    (((u32::from(numerator)) << 14) / u32::from(denominator)) as u16
                };
                let index = (ratio >> 5) & 0xFFFE;
                let table = self.memory.read_long_word(0x0D_FC74 + u32::from(index));
                if swapped {
                    0x4000u16.wrapping_sub(table)
                } else {
                    table
                }
            }
        };
        if ((original_x ^ original_y) as i16) < 0 {
            angle = angle.wrapping_neg();
        }
        if (original_y as i16) < 0 {
            angle = angle.wrapping_add(0x8000);
        }
        angle
    }

    fn sf2_target_table_curve(&self, value: u16, negative: bool, carry: bool) -> u16 {
        let index = if negative {
            value.wrapping_neg() & 0x001F
        } else {
            value & 0x001F
        };
        let use_upper_table = if negative { !carry } else { carry };
        let base = if use_upper_table {
            0x07_B4DD
        } else {
            0x07_B4BD
        };
        u16::from(self.memory.read_long_byte(base + u32::from(index)))
    }

    /// Retail `$07:B1EA/$07:B1FD`, excluding the small path-handler wrapper.
    fn update_player_target_retail(&mut self, target: u16, mode: u8) {
        self.memory.write_byte(0x1DB2, mode);
        self.memory.write_word(0x1DC0, target);

        let player = self.memory.read_word(PLAYER_ONE);
        let slot = self.memory.read_word(player.wrapping_add(FIELD_PATH));
        if self.memory.read_byte(0x6BC2u16.wrapping_add(slot)) & 0x10 != 0 {
            return;
        }

        let flags = self.memory.read_byte(0x6BB6u16.wrapping_add(slot)) & 0x7F;
        self.memory.write_byte(0x6BB6u16.wrapping_add(slot), flags);
        self.memory
            .write_word(0x12DE, self.memory.read_word(0x6BBAu16.wrapping_add(slot)));

        let anchor = 0x033F;
        let dx = self
            .object_word(anchor, FIELD_X)
            .wrapping_sub(self.object_word(target, FIELD_X));
        let dz = self
            .object_word(anchor, FIELD_Z)
            .wrapping_sub(self.object_word(target, FIELD_Z));
        let distance = sf2_xz_angle_distance(dx, dz) as u16;
        self.memory.write_word(0x12DE, distance);
        self.memory.write_word(0x1DB4, distance);

        // `$07:AE90`: normalize the half-coordinate X/Z Manhattan distance,
        // scale the vertical component by the same amount, then calculate the
        // pitch and yaw relative to the fixed camera/target anchor at $033F.
        let half_target_x = sf2_signed_half(self.memory.read_word(target + FIELD_X));
        let half_anchor_x = sf2_signed_half(self.memory.read_word(anchor + FIELD_X));
        let half_target_z = sf2_signed_half(self.memory.read_word(target + FIELD_Z));
        let half_anchor_z = sf2_signed_half(self.memory.read_word(anchor + FIELD_Z));
        let x_abs = sf2_abs_word(half_target_x.wrapping_sub(half_anchor_x));
        let z_abs = sf2_abs_word(half_target_z.wrapping_sub(half_anchor_z));
        let mut normalized = x_abs.wrapping_add(z_abs);
        let mut shifts = 0u16;
        while normalized & 0xF000 != 0 {
            normalized >>= 1;
            shifts = shifts.wrapping_add(1);
        }
        self.memory.write_word(0x12DE, normalized);
        self.memory.write_word(0x3A, shifts);
        self.memory.write_word(0x08, normalized);
        self.memory.write_word(0x149D, 0);

        let half_target_y = sf2_signed_half(self.memory.read_word(target + FIELD_Y));
        let half_anchor_y = sf2_signed_half(self.memory.read_word(anchor + FIELD_Y));
        let mut vertical = half_target_y.wrapping_sub(half_anchor_y);
        for _ in 0..shifts {
            vertical = sf2_signed_half(vertical);
        }
        self.memory.write_word(0x02, vertical);
        self.memory.write_word(0x3C, half_anchor_y);
        let pitch = self
            .sf2_atan16(vertical, normalized)
            .wrapping_add(self.memory.read_word(anchor + FIELD_ROT_X));
        self.memory.write_word(0x1DAE, pitch);

        let yaw_dx = self
            .memory
            .read_word(target + FIELD_X)
            .wrapping_sub(self.memory.read_word(anchor + FIELD_X));
        let yaw_dz = self
            .memory
            .read_word(target + FIELD_Z)
            .wrapping_sub(self.memory.read_word(anchor + FIELD_Z));
        self.memory.write_word(0x02, yaw_dx);
        self.memory.write_word(0x08, yaw_dz);
        let mut aim_yaw = self
            .sf2_atan16(yaw_dx, yaw_dz)
            .wrapping_neg()
            .wrapping_add(self.memory.read_word(anchor + FIELD_ROT_Y));
        self.memory.write_word(0x1DB0, aim_yaw);

        // AE90 finishes with an intentionally byte-sized Manhattan distance;
        // SEP #$20 leaves the normalized distance's high byte untouched.
        let low_abs = |value: u8| {
            if value as i8 >= 0 {
                value
            } else {
                value.wrapping_neg()
            }
        };
        let low_distance = low_abs(
            self.memory
                .read_byte(target + FIELD_X)
                .wrapping_sub(self.memory.read_byte(anchor + FIELD_X)),
        )
        .wrapping_add(low_abs(
            self.memory
                .read_byte(target + FIELD_Z)
                .wrapping_sub(self.memory.read_byte(anchor + FIELD_Z)),
        ));
        self.memory.write_byte(0x12DE, low_distance);

        self.memory.write_word(0x1DBA, aim_yaw);
        self.memory.write_word(0x1DB8, pitch);

        // `$07:B256..B32C`: quantize the two 16-bit angles into the sparse
        // per-player display/control bytes using the original interpolation
        // tables.  The unaligned reads are deliberate and reproduce the 65816.
        let yaw = aim_yaw;
        let display_x = if yaw as i16 >= 0 && yaw >= self.memory.read_long_word(0x07_B481) {
            aim_yaw = aim_yaw.wrapping_add(1);
            self.memory.read_long_word(0x07_B47D)
        } else if (yaw as i16) < 0 && yaw < self.memory.read_long_word(0x07_B483) {
            aim_yaw = aim_yaw.wrapping_add(1);
            self.memory.read_long_word(0x07_B47F)
        } else {
            let unaligned = self.memory.read_word(0x1DB9);
            let carry = unaligned & 0x8000 != 0;
            let argument = self.memory.read_word(0x1DBB);
            let curve = self.sf2_target_table_curve(argument, (yaw as i16) < 0, carry);
            let signed_curve = if (yaw as i16) < 0 {
                curve
            } else {
                curve.wrapping_neg()
            };
            signed_curve.wrapping_add(0x0070)
        };
        self.memory.write_word(0x79, display_x);

        let group = if (yaw as i16) < 0 && yaw < 0xEE00 {
            0x10u32
        } else if yaw as i16 >= 0 && yaw >= 0x1500 {
            0x08
        } else {
            0
        };
        let display_y =
            if pitch as i16 >= 0 && pitch >= self.memory.read_long_word(0x07_B489 + group) {
                aim_yaw = aim_yaw.wrapping_add(1);
                self.memory.read_long_word(0x07_B485 + group)
            } else if (pitch as i16) < 0 && pitch < self.memory.read_long_word(0x07_B48B + group) {
                aim_yaw = aim_yaw.wrapping_add(1);
                self.memory.read_long_word(0x07_B487 + group)
            } else {
                let unaligned = self.memory.read_word(0x1DB9);
                let carry = unaligned & 0x8000 != 0;
                let mut curve = self.sf2_target_table_curve(unaligned, (pitch as i16) < 0, carry);
                if (pitch as i16) < 0 {
                    curve = curve.wrapping_neg();
                }
                let quarter = sf2_signed_half(sf2_signed_half(curve));
                let mut shaped = curve.wrapping_add(quarter);
                let negative_eighth = sf2_signed_half(quarter).wrapping_neg();
                shaped = shaped.wrapping_add(negative_eighth);
                shaped
                    .wrapping_add(sf2_signed_half(negative_eighth))
                    .wrapping_add(0x0060)
            };
        self.memory.write_word(0x7B, display_y);
        self.memory.write_word(0x1DB0, aim_yaw);

        let owner = self.memory.read_word(0x6BCAu16.wrapping_add(slot));
        let force = owner != 0 && owner == target;
        if force {
            let flags = self.memory.read_byte(0x6BC2u16.wrapping_add(slot)) | 0x10;
            self.memory.write_byte(0x6BC2u16.wrapping_add(slot), flags);
        } else if sf2_abs_word(distance) >= self.memory.read_word(0x6BBCu16.wrapping_add(slot)) {
            return;
        }

        for (address, value) in [
            (0x6BBCu16, distance),
            (0x6BBA, self.memory.read_word(0x12DE)),
            (0x6BB8, target),
            (0x6BCC, self.memory.read_word(target + FIELD_X)),
            (0x6BCE, self.memory.read_word(target + FIELD_Y)),
            (0x6BD0, self.memory.read_word(target + FIELD_Z)),
            (0x6BBE, yaw),
            (0x6BC0, pitch),
        ] {
            self.memory.write_word(address.wrapping_add(slot), value);
        }
        self.memory
            .write_byte(0x6BADu16.wrapping_add(slot), display_x as u8);
        self.memory
            .write_byte(0x6BAFu16.wrapping_add(slot), display_y as u8);
        self.memory
            .write_byte(0x6BC5u16.wrapping_add(slot), aim_yaw as u8);
        let flags = self.memory.read_byte(0x6BC2u16.wrapping_add(slot)) | mode;
        self.memory.write_byte(0x6BC2u16.wrapping_add(slot), flags);
    }
}

impl Sf2PathHost for Game {
    type Error = Error;

    fn read_variable_byte(&self, id: u8) -> Result<u8, Self::Error> {
        Ok(self.memory.read_byte(self.variable_address(id)?))
    }

    fn write_variable_byte(&mut self, id: u8, value: u8) -> Result<(), Self::Error> {
        let address = self.variable_address(id)?;
        self.memory.write_byte(address, value);
        Ok(())
    }

    fn read_variable_word(&self, id: u8) -> Result<u16, Self::Error> {
        Ok(self.memory.read_word(self.variable_address(id)?))
    }

    fn write_variable_word(&mut self, id: u8, value: u16) -> Result<(), Self::Error> {
        let address = self.variable_address(id)?;
        self.memory.write_word(address, value);
        Ok(())
    }

    fn read_external_byte(&self, address: u16) -> Result<u8, Self::Error> {
        Ok(self.memory.read_byte(address))
    }

    fn write_external_byte(&mut self, address: u16, value: u8) -> Result<(), Self::Error> {
        self.memory.write_byte(address, value);
        Ok(())
    }

    fn read_external_word(&self, address: u16) -> Result<u16, Self::Error> {
        Ok(self.memory.read_word(address))
    }

    fn write_external_word(&mut self, address: u16, value: u16) -> Result<(), Self::Error> {
        self.memory.write_word(address, value);
        Ok(())
    }

    fn read_external_long_byte(&self, address: u32) -> Result<u8, Self::Error> {
        Ok(self.memory.read_long_byte(address))
    }

    fn read_external_long_word(&self, address: u32) -> Result<u16, Self::Error> {
        Ok(self.memory.read_long_word(address))
    }

    fn read_object_extension_byte(&self, offset: u16) -> Result<u8, Self::Error> {
        Ok(self
            .memory
            .read_byte(self.current_object()?.wrapping_add(offset)))
    }

    fn write_object_extension_byte(&mut self, offset: u16, value: u8) -> Result<(), Self::Error> {
        let address = self.current_object()?.wrapping_add(offset);
        self.memory.write_byte(address, value);
        Ok(())
    }

    fn read_object_extension_word(&self, offset: u16) -> Result<u16, Self::Error> {
        Ok(self
            .memory
            .read_word(self.current_object()?.wrapping_add(offset)))
    }

    fn write_object_extension_word(&mut self, offset: u16, value: u16) -> Result<(), Self::Error> {
        let address = self.current_object()?.wrapping_add(offset);
        self.memory.write_word(address, value);
        Ok(())
    }

    fn find_shape(&mut self, shape: u16) -> Result<(), Self::Error> {
        let found = active_objects(&self.memory)
            .into_iter()
            .find(|object| self.memory.read_word(*object + FIELD_SHAPE) == shape)
            .unwrap_or(0);
        let current = self.current_object()?;
        self.memory.write_word(current + 0x06, found);
        Ok(())
    }

    fn pointed_shape_is_dead(&self) -> Result<bool, Self::Error> {
        Ok(self.memory.read_word(self.current_object()? + 0x06) == 0)
    }

    fn child_is_dead(&mut self, child_number: u8) -> Result<bool, Self::Error> {
        let child = self.find_child(child_number)?;
        self.memory.write_word(SELECTED_OBJECT, child);
        Ok(child == 0)
    }

    fn flag_child(&mut self, child_number: u8) -> Result<(), Self::Error> {
        if !self.child_is_dead(child_number)? {
            if let Some(child) = self.selected_object() {
                let value = self.memory.read_byte(child + 0x23) | 0x08;
                self.memory.write_byte(child + 0x23, value);
            }
        }
        Ok(())
    }

    fn fire_weapon(&mut self) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        for address in [0x14B0u16, 0x14B2, 0x14B4, 0x14B7, 0x14B6, 0x14B8, 0x14B9] {
            self.memory.write_byte(address, 0);
        }
        let selector = self.memory.read_byte(current + 0x2F);
        let target = self
            .spawn_path_weapon(current, selector)
            .unwrap_or_else(|| self.memory.read_word(0x14D6));
        self.memory.write_word(0xD771, target);
        if object_index(target).is_some() {
            let flags = self.memory.read_byte(target + 0x31) | 0x10;
            self.memory.write_byte(target + 0x31, flags);
        }
        // The retail dispatcher returns Y but preserves the path owner's X.
        self.memory.write_word(CURRENT_OBJECT, current);
        Ok(())
    }

    fn face_player(&mut self) -> Result<(), Self::Error> {
        let selected = self.memory.read_word(SELECTED_OBJECT);
        let target = if selected == self.memory.read_word(PLAYER_ONE) {
            0x033F
        } else {
            0x037E
        };
        let current = self.current_object()?;
        self.face_object(target, None)?;
        self.refresh_relative_yaw(current);
        Ok(())
    }

    fn face_player_yaw(&mut self) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        if let Some(selected) = self.selected_object() {
            let (dx, _, dz) = self.object_delta(current, selected);
            let target = sf_core::aim_angle::yanglexy(dx, dz).wrapping_neg();
            let address = current + FIELD_ROT_Y;
            let mut yaw = self.memory.read_byte(address);
            sf_core::snes_trig::achase_angle_8(&mut yaw, target, 2);
            self.memory.write_byte(address, yaw);
            self.refresh_relative_yaw(current);
        }
        Ok(())
    }

    fn face_mother(&mut self) -> Result<(), Self::Error> {
        let mother = self.memory.read_word(self.current_object()? + 0x06);
        if object_index(mother).is_some() {
            self.face_object(mother, None)?;
        }
        Ok(())
    }

    fn copy_selected_world_position(&mut self) -> Result<(), Self::Error> {
        if let Some(selected) = self.selected_object() {
            let current = self.current_object()?;
            for field in [FIELD_X, FIELD_Y, FIELD_Z] {
                let value = self.memory.read_word(selected + field);
                self.memory.write_word(current + field, value);
            }
        }
        Ok(())
    }

    fn enter_path_hold(&mut self) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        let flags = self.memory.read_byte(current + 0x09) | 0x08;
        self.memory.write_byte(current + 0x09, flags);
        self.memory.write_word(current + FIELD_STRATEGY, 0x9DDE);
        self.memory.write_byte(current + FIELD_STRATEGY + 2, 0x7F);
        Ok(())
    }

    fn selected_distance(&mut self) -> Result<u16, Self::Error> {
        let Some(selected) = self.selected_object() else {
            return Ok(u16::MAX);
        };
        let (x, _, z) = self.object_delta(self.current_object()?, selected);
        let square = i64::from(x) * i64::from(x) + i64::from(z) * i64::from(z);
        Ok((square as u64).isqrt().min(u64::from(u16::MAX)) as u16)
    }

    fn mother_distance(&mut self) -> Result<Option<u16>, Self::Error> {
        let mother = self.memory.read_word(self.current_object()? + 0x06);
        if object_index(mother).is_none() {
            return Ok(None);
        }
        self.memory.write_word(SELECTED_OBJECT, mother);
        self.selected_distance().map(Some)
    }

    fn selected_within_range(&mut self, range: u16) -> Result<bool, Self::Error> {
        Ok(self.selected_distance()? < range)
    }

    fn selected_relative_yaw(&mut self) -> Result<u8, Self::Error> {
        // Retail `$7F:AB7E` uses `$CF1F` directly as a WRAM base.  Zero is a
        // valid base here (and is used before a target has been selected), so
        // do not apply the object-pool validity check used by higher-level
        // object operations.
        let selected = self.memory.read_word(SELECTED_OBJECT);
        let current = self.current_object()?;
        let dx = self
            .object_word(current, FIELD_X)
            .wrapping_sub(self.object_word(selected, FIELD_X));
        let dz = self
            .object_word(current, FIELD_Z)
            .wrapping_sub(self.object_word(selected, FIELD_Z));
        Ok(((self.sf2_atan16(dx as u16, dz as u16) >> 8) as u8)
            .wrapping_add(self.memory.read_byte(selected + FIELD_ROT_Y)))
    }

    fn selected_bearing_plus_yaw(&mut self) -> Result<u8, Self::Error> {
        // `$7F:AB48` has the same raw `$CF1F` addressing behavior as `$AB7E`,
        // but computes the opposite vector (selected minus current).
        let selected = self.memory.read_word(SELECTED_OBJECT);
        let current = self.current_object()?;
        let dx = self
            .object_word(selected, FIELD_X)
            .wrapping_sub(self.object_word(current, FIELD_X));
        let dz = self
            .object_word(selected, FIELD_Z)
            .wrapping_sub(self.object_word(current, FIELD_Z));
        Ok(((self.sf2_atan16(dx as u16, dz as u16) >> 8) as u8)
            .wrapping_add(self.memory.read_byte(current + FIELD_ROT_Y)))
    }

    fn rotate_around_selected_yaw(&mut self, angle: i8) -> Result<(), Self::Error> {
        let selected = self.selected_object().ok_or(Error::InvalidObject(0))?;
        let current = self.current_object()?;
        let dx = self
            .object_word(current, FIELD_X)
            .wrapping_sub(self.object_word(selected, FIELD_X));
        let dz = self
            .object_word(current, FIELD_Z)
            .wrapping_sub(self.object_word(selected, FIELD_Z));
        let (x, z) = sf_core::snes_trig::rotate_16xz(angle as u8, dx, dz);
        self.set_object_word(
            current,
            FIELD_X,
            self.object_word(selected, FIELD_X).wrapping_add(x),
        );
        self.set_object_word(
            current,
            FIELD_Z,
            self.object_word(selected, FIELD_Z).wrapping_add(z),
        );
        Ok(())
    }

    fn rotate_around_selected_pitch(&mut self, angle: i8) -> Result<(), Self::Error> {
        let selected = self.selected_object().ok_or(Error::InvalidObject(0))?;
        let current = self.current_object()?;
        let dy = self
            .object_word(current, FIELD_Y)
            .wrapping_sub(self.object_word(selected, FIELD_Y));
        let dz = self
            .object_word(current, FIELD_Z)
            .wrapping_sub(self.object_word(selected, FIELD_Z));
        let (y, z) = sf_core::snes_trig::rotate_16yz(angle as u8, dy, dz);
        self.set_object_word(
            current,
            FIELD_Y,
            self.object_word(selected, FIELD_Y).wrapping_add(y),
        );
        self.set_object_word(
            current,
            FIELD_Z,
            self.object_word(selected, FIELD_Z).wrapping_add(z),
        );
        Ok(())
    }

    fn try_transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<bool, Self::Error> {
        let target = match transition {
            ContextTransition::BecomeMother => self.memory.read_word(self.current_object()? + 0x06),
            ContextTransition::BecomeChild(number) => active_objects(&self.memory)
                .into_iter()
                .find(|object| {
                    self.memory.read_word(*object + 0x06) == self.current_object().unwrap_or(0)
                        && self.memory.read_byte(*object + 0x13) == number
                })
                .unwrap_or(0),
            _ => 0,
        };
        if object_index(target).is_none() {
            return Ok(false);
        }
        let current = self.current_object()?;
        self.memory.write_word(0xD76D, current);
        self.memory
            .write_word(0xD76F, self.memory.read_word(target + FIELD_PATH));
        self.memory
            .write_word(current + FIELD_PATH, resume_at.offset);
        self.memory
            .write_word(target + FIELD_PATH, resume_at.offset);
        self.memory.write_word(CURRENT_OBJECT, target);
        Ok(true)
    }

    fn selected_slot_class(&self) -> Result<u8, Self::Error> {
        Ok(self
            .selected_object()
            .map(|object| self.memory.read_byte(object + FIELD_PATH + 1))
            .unwrap_or(0))
    }

    fn selected_aux_flags(&self) -> Result<u8, Self::Error> {
        Ok(self.memory.read_byte(self.selected_aux_address(0x6B63)?))
    }

    fn or_selected_aux_flags(&mut self, bits: u8) -> Result<(), Self::Error> {
        let address = self.selected_aux_address(0x6B63)?;
        let value = self.memory.read_byte(address) | bits;
        self.memory.write_byte(address, value);
        Ok(())
    }

    fn set_selected_slot_low_nibble_4(&mut self) -> Result<(), Self::Error> {
        // `$7F:B04D` deliberately does not validate `$CF1F`: it swaps the
        // raw value into X, reads that record's word `$2B` as a sparse pilot
        // slot index, and changes `$6AA1+slot`. A zero selected pointer thus
        // uses the direct-page word at `$002B`, exactly as retail does.
        let selected = self.memory.read_word(SELECTED_OBJECT);
        let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
        let address = 0x6AA1u16.wrapping_add(slot);
        let value = self.memory.read_byte(address);
        self.memory.write_byte(address, (value & 0xF0) | 4);
        Ok(())
    }

    fn allocate_auxiliary_type_0b(&mut self, value: u8) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        let entry = get_or_create_auxiliary_type(&mut self.memory, current, 0x0B)
            .ok_or(Error::AuxiliaryHeapExhausted)?;
        write_auxiliary_byte(&mut self.memory, entry + 1, value);
        Ok(())
    }

    fn allocate_auxiliary_type_0d(&mut self, value: u8) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        let entry = get_or_create_auxiliary_type(&mut self.memory, current, 0x0D)
            .ok_or(Error::AuxiliaryHeapExhausted)?;
        write_auxiliary_byte(&mut self.memory, entry + 1, value);
        Ok(())
    }

    fn perform_path_operation(&mut self, operation: Sf2PathOperation) -> Result<(), Self::Error> {
        match operation {
            Sf2PathOperation::FaceSelectedSmooth => {
                if let Some(selected) = self.selected_object() {
                    let current = self.current_object()?;
                    self.face_object(selected, Some(2))?;
                    self.refresh_relative_yaw(current);
                }
            }
            Sf2PathOperation::FaceLinkedSmooth => {
                let linked = self.memory.read_word(self.current_object()? + 0x06);
                if object_index(linked).is_some() {
                    self.face_object(linked, Some(3))?;
                }
            }
            Sf2PathOperation::ExplodeObject => {
                let current = self.current_object()?;
                let sound_slot = self.memory.read_byte(current + 0x28);
                if sound_slot != 0 {
                    self.memory
                        .write_byte(0x17FBu16.wrapping_add(u16::from(sound_slot)), 0);
                }
                if self.memory.read_byte(current + 0x23) & 0x10 != 0 {
                    let mut child = self.memory.read_word(current + 0x29);
                    while child != 0 {
                        let next = self.memory.read_word(child + 0x29);
                        let flags = self.memory.read_byte(child + 0x21) | 0x01;
                        self.memory.write_byte(child + 0x21, flags);
                        self.memory.write_byte(child + 0x2D, 0);
                        child = next;
                    }
                }
                let flags = self.memory.read_byte(current + 0x21) | 0x01;
                self.memory.write_byte(current + 0x21, flags);
                self.memory.write_byte(current + 0x2D, 0);
            }
            Sf2PathOperation::SpawnObject(spawn) => {
                self.spawn_full(spawn)?;
            }
            Sf2PathOperation::FlagLinkedObject => {
                let linked = self.memory.read_word(self.current_object()? + 0x06);
                if object_index(linked).is_some() {
                    let value = self.memory.read_byte(linked + 0x23) | 0x08;
                    self.memory.write_byte(linked + 0x23, value);
                }
            }
            Sf2PathOperation::UnlinkChild(number) => {
                if !self.child_is_dead(number)? {
                    if let Some(child) = self.selected_object() {
                        let flags_23 = self.memory.read_byte(child + 0x23) & !0x04;
                        let flags_25 = self.memory.read_byte(child + 0x25) & !0x01;
                        self.memory.write_byte(child + 0x23, flags_23);
                        self.memory.write_byte(child + 0x25, flags_25);
                        let mother = self.memory.read_word(child + 0x06);
                        if mother != 0 {
                            let mut predecessor = mother;
                            while self.memory.read_word(predecessor + 0x29) != 0 {
                                let next = self.memory.read_word(predecessor + 0x29);
                                if next == child {
                                    let sibling = self.memory.read_word(child + 0x29);
                                    self.memory.write_word(predecessor + 0x29, sibling);
                                    break;
                                }
                                predecessor = next;
                            }
                        }
                        self.memory.write_word(child + 0x06, 0);
                        self.memory.write_word(child + 0x29, 0);
                        self.memory.write_byte(child + 0x13, 0);
                    }
                }
            }
            Sf2PathOperation::AccumulateObject1cde(variable) => {
                let current = self.current_object()?;
                let address = current.wrapping_add(0x1CDE);
                let value = self
                    .memory
                    .read_word(address)
                    .wrapping_add(self.memory.read_word(current + 0x02))
                    .wrapping_add(self.read_variable_word(variable)?);
                self.memory.write_word(address, value);
            }
            Sf2PathOperation::SaturatingAddSelectedAuxWord(value) => {
                let address = self.selected_aux_address(0x6C33)?;
                self.memory.write_word(
                    address,
                    self.memory.read_word(address).saturating_add(value),
                );
            }
            Sf2PathOperation::RefreshSelectedRelativeTransform => {
                if let Some(selected) = self.selected_object() {
                    let current = self.current_object()?;
                    let (x, y, z) = self.object_delta(selected, current);
                    self.memory
                        .write_word(current.wrapping_add(0x1CD8), selected);
                    let mut inverse_angles = [0u8; 3];
                    for (index, (field, offset)) in [
                        (FIELD_ROT_X, 0x1CD5),
                        (FIELD_ROT_Y, 0x1CD6),
                        (FIELD_ROT_Z, 0x1CD7),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let delta = self
                            .memory
                            .read_byte(current + field)
                            .wrapping_sub(self.memory.read_byte(selected + field));
                        self.memory.write_byte(current.wrapping_add(offset), delta);
                        inverse_angles[index] =
                            self.memory.read_byte(selected + field).wrapping_neg();
                    }
                    let matrix = sf_core::snes_trig::zxy_matrix_q15(
                        inverse_angles[0],
                        inverse_angles[1],
                        inverse_angles[2],
                    );
                    for (index, value) in matrix.into_iter().flatten().enumerate() {
                        let offset = (index as u16) * 2;
                        self.memory.write_word(0x153E + offset, value as u16);
                        self.memory.write_word(0x157C + offset, value as u16);
                    }
                    let (x, y, z) = sf_core::snes_trig::matrix_rotate_q15(matrix, x, y, z);
                    for (offset, value) in [(0x1CCF, x), (0x1CD1, y), (0x1CD3, z)] {
                        self.memory
                            .write_word(current.wrapping_add(offset), value as u16);
                    }
                    let flags = self.memory.read_byte(current + 0x25) | 0x04;
                    self.memory.write_byte(current + 0x25, flags);
                }
            }
            Sf2PathOperation::SelectSelfAndClearRelativeTransform => {
                let current = self.current_object()?;
                self.memory
                    .write_word(current.wrapping_add(0x1CD8), current);
                for offset in [0x1CCF, 0x1CD1, 0x1CD3] {
                    self.memory.write_word(current.wrapping_add(offset), 0);
                }
                for offset in [0x1CD5, 0x1CD6, 0x1CD7] {
                    self.memory.write_byte(current.wrapping_add(offset), 0);
                }
            }
            Sf2PathOperation::InitializePlayerAuxWord(value) => {
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    let address = 0x6B3Bu16.wrapping_add(slot);
                    if self.memory.read_word(address) == 0 {
                        self.memory.write_word(address, value);
                    }
                }
            }
            Sf2PathOperation::SetPlayerAuxMode(enabled) => {
                self.set_player_aux_mode(enabled)?;
            }
            Sf2PathOperation::RefreshLinkedRotationDeltas => {
                let current = self.current_object()?;
                let linked = self.memory.read_word(current + 0x06);
                if object_index(linked).is_some() {
                    for (field, offset) in [
                        (FIELD_ROT_X, 0x1CD5),
                        (FIELD_ROT_Y, 0x1CD6),
                        (FIELD_ROT_Z, 0x1CD7),
                    ] {
                        let delta = self
                            .memory
                            .read_byte(current + field)
                            .wrapping_sub(self.memory.read_byte(linked + field));
                        self.memory.write_byte(current.wrapping_add(offset), delta);
                    }
                }
            }
            Sf2PathOperation::UpdatePilotAuxState => {
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    if self.memory.read_word(0x6C1C + slot) == 8
                        && self.memory.read_byte(0x6C00 + slot) != 0
                    {
                        self.memory.write_byte(0x6C11 + slot, 4);
                        let flags = self.memory.read_byte(0x6C12 + slot) | 0x24;
                        self.memory.write_byte(0x6C12 + slot, flags);
                    }
                }
            }
            Sf2PathOperation::QueueFixedMarker1400(value) => self.map_markers.push(MapMarker {
                kind: value,
                table_index: 0x1400,
            }),
            Sf2PathOperation::QueueFixedMarker0320(value) => self.map_markers.push(MapMarker {
                kind: value,
                table_index: 0x0320,
            }),
            Sf2PathOperation::QueueSelectedMarkerPair { first, second } => {
                let selected = self.memory.read_word(SELECTED_OBJECT);
                self.enqueue_event(u16::from_le_bytes([first, second]), selected);
                self.map_markers.push(MapMarker {
                    kind: first,
                    table_index: u16::from(second),
                });
            }
            Sf2PathOperation::FaceSelectedImmediate => {
                if let Some(selected) = self.selected_object() {
                    let current = self.current_object()?;
                    self.face_object(selected, None)?;
                    self.refresh_relative_yaw(current);
                }
            }
            Sf2PathOperation::ChasePlayerTowardObject => {
                let current = self.current_object()?;
                let player_anchor = 0x033F;
                for field in [FIELD_X, FIELD_Y, FIELD_Z, FIELD_Z] {
                    let from = self.object_word(player_anchor, field);
                    let target = self.object_word(current, field);
                    self.set_object_word(
                        player_anchor,
                        field,
                        Self::chase_word(from, target, 3, 8),
                    );
                }
                let from = self.object_word(player_anchor, 0x29);
                self.set_object_word(player_anchor, 0x29, Self::chase_word(from, 0, 3, 8));
                for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
                    let angle = self.memory.read_byte(current + field);
                    let target = ((u16::from(angle) << 8) as i16).wrapping_neg();
                    let from = self.object_word(player_anchor, field);
                    self.set_object_word(
                        player_anchor,
                        field,
                        Self::chase_word(from, target, 3, 8),
                    );
                }
            }
            Sf2PathOperation::SnapPlayerToObject => {
                let current = self.current_object()?;
                let player_anchor = 0x033F;
                for field in [FIELD_X, FIELD_Y, FIELD_Z] {
                    let value = self.memory.read_word(current + field);
                    self.memory.write_word(player_anchor + field, value);
                }
                self.memory.write_word(player_anchor + 0x29, 0);
                for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
                    let angle = self.memory.read_byte(current + field);
                    let value = (u16::from(angle) << 8).wrapping_neg();
                    self.memory.write_word(player_anchor + field, value);
                }
            }
            Sf2PathOperation::RotateAroundLinkedPitch(angle) => {
                let linked = self.memory.read_word(self.current_object()? + 0x06);
                if object_index(linked).is_some() {
                    let old = self.memory.read_word(SELECTED_OBJECT);
                    self.memory.write_word(SELECTED_OBJECT, linked);
                    self.rotate_around_selected_pitch(angle)?;
                    self.memory.write_word(SELECTED_OBJECT, old);
                }
            }
            Sf2PathOperation::RotateLocalOffsetYaw(angle) => {
                let current = self.current_object()?;
                let x = self.memory.read_word(current.wrapping_add(0x1CCF)) as i16;
                let z = self.memory.read_word(current.wrapping_add(0x1CD3)) as i16;
                let (x, z) = sf_core::snes_trig::rotate_16xz(angle as u8, x, z);
                self.memory
                    .write_word(current.wrapping_add(0x1CCF), x as u16);
                self.memory
                    .write_word(current.wrapping_add(0x1CD3), z as u16);
            }
            Sf2PathOperation::RotateLocalOffsetPitch(angle) => {
                let current = self.current_object()?;
                let y = self.memory.read_word(current.wrapping_add(0x1CD1)) as i16;
                let z = self.memory.read_word(current.wrapping_add(0x1CD3)) as i16;
                let (y, z) = sf_core::snes_trig::rotate_16yz(angle as u8, y, z);
                self.memory
                    .write_word(current.wrapping_add(0x1CD1), y as u16);
                self.memory
                    .write_word(current.wrapping_add(0x1CD3), z as u16);
            }
            Sf2PathOperation::UnlinkSelf => {
                let current = self.current_object()?;
                if self.memory.read_byte(current + 0x23) & 0x04 != 0 {
                    let mother = self.memory.read_word(current + 0x06);
                    let flags_23 = self.memory.read_byte(current + 0x23) & !0x04;
                    let flags_25 = self.memory.read_byte(current + 0x25) & !0x01;
                    self.memory.write_byte(current + 0x23, flags_23);
                    self.memory.write_byte(current + 0x25, flags_25);

                    let mut predecessor = mother;
                    while predecessor != 0 {
                        let next = self.memory.read_word(predecessor + 0x29);
                        if next == 0 {
                            break;
                        }
                        if next == current {
                            let sibling = self.memory.read_word(current + 0x29);
                            self.memory.write_word(predecessor + 0x29, sibling);
                            self.memory.write_word(current + 0x06, 0);
                            self.memory.write_word(current + 0x29, 0);
                            self.memory.write_byte(current + 0x13, 0);
                            break;
                        }
                        predecessor = next;
                    }
                }
            }
            Sf2PathOperation::PositionRelativeToLinked(distance) => {
                let current = self.current_object()?;
                let linked = self.memory.read_word(current + 0x06);
                if object_index(linked).is_some() {
                    let (dx, dy, dz) = self.object_delta(current, linked);
                    let pitch = (self.sf2_atan16(dy as u16, sf2_xz_angle_distance(dx, dz) as u16)
                        >> 8) as u8;
                    let yaw = (self.sf2_atan16(dx as u16, dz as u16).wrapping_neg() >> 8) as u8;
                    self.memory.write_byte(current + FIELD_ROT_X, pitch);
                    self.memory.write_byte(current + FIELD_ROT_Y, yaw);

                    let matrix = sf_core::snes_trig::zxy_matrix_q15(
                        self.memory.read_byte(linked + FIELD_ROT_X),
                        self.memory.read_byte(linked + FIELD_ROT_Y),
                        0,
                    );
                    let (x, y, z) =
                        sf_core::snes_trig::matrix_rotate_q15(matrix, 0, 0, i16::from(distance));
                    for (field, delta) in [
                        (FIELD_X, x.wrapping_shl(3)),
                        (FIELD_Y, y.wrapping_shl(3)),
                        (FIELD_Z, z.wrapping_shl(3)),
                    ] {
                        self.set_object_word(
                            current,
                            field,
                            self.object_word(linked, field).wrapping_add(delta),
                        );
                    }
                }
            }
            Sf2PathOperation::CopySelectedSlotWorldPosition => {
                let selected = self.memory.read_word(SELECTED_OBJECT);
                let current = self.current_object()?;
                let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
                for (field, base) in [
                    (FIELD_X, 0x6AC1u16),
                    (FIELD_Y, 0x6AC3u16),
                    (FIELD_Z, 0x6AC5u16),
                ] {
                    let value = self.memory.read_word(base.wrapping_add(slot));
                    self.memory.write_word(current + field, value);
                }
            }
            Sf2PathOperation::PositionExternalObjectAndFaceSelected => {
                if let Some(selected) = self.selected_object() {
                    let external = self.memory.read_word(0x14D6);
                    if object_index(external).is_some() {
                        let offset_x = self.memory.read_byte(0x16B1) as i8;
                        let offset_y = self.memory.read_byte(0x16B3) as i8;
                        let offset_z = self.memory.read_byte(0x16B5) as i8;
                        let yaw = self.memory.read_byte(selected + FIELD_ROT_Y);
                        let (x, z) = sf_core::snes_trig::rotate_8xz(yaw, offset_x, offset_z);
                        for (field, offset) in
                            [(FIELD_X, x), (FIELD_Y, i16::from(offset_y)), (FIELD_Z, z)]
                        {
                            let value = self.object_word(selected, field).wrapping_add(offset << 4);
                            self.set_object_word(external, field, value);
                        }
                    }
                    self.memory.write_byte(0x149D, 0);
                    self.face_object(selected, Some(3))?;
                }
            }
            Sf2PathOperation::CallExternalStrategy(strategy) => {
                let current = self.current_object()?;
                let index = u32::from((strategy & 0x0F).min(5)) * 4;
                self.memory.write_word(
                    current + FIELD_SHAPE,
                    self.memory.read_long_word(0x068135 + index),
                );
                self.memory.write_word(
                    current.wrapping_add(0x1CCD),
                    self.memory.read_long_word(0x068137 + index),
                );
            }
            Sf2PathOperation::CopySelectedAuxRotation => {
                let selected = self.selected_object().ok_or(Error::InvalidObject(0))?;
                let current = self.current_object()?;
                for (field, base) in [
                    (FIELD_ROT_X, 0x6B32u16),
                    (FIELD_ROT_Y, 0x6B34u16),
                    (FIELD_ROT_Z, 0x6B36u16),
                ] {
                    let address = base.wrapping_add(self.memory.read_word(selected + FIELD_PATH));
                    let value = self.memory.read_byte(address);
                    self.memory.write_byte(current + field, value);
                }
            }
            Sf2PathOperation::ConfigurePilotAuxModeA(value) => {
                self.configure_pilot_auxiliary(value, PilotAuxiliaryMode::DoubledValue)?;
            }
            Sf2PathOperation::ConfigurePilotAuxModeB(value) => {
                self.configure_pilot_auxiliary(value, PilotAuxiliaryMode::AlternateAxes)?;
            }
            Sf2PathOperation::ApplyFormationOffset => {
                let current = self.current_object()?;
                let c4 = self.memory.read_word(0x00C4);
                let xz_index =
                    u16::from((current as u8).wrapping_add(c4 as u8).wrapping_mul(2) & 0x3E);
                let xz_offset = self.memory.read_long_word(0x7FC306 + u32::from(xz_index)) as i16;
                for field in [FIELD_X, FIELD_Z] {
                    let value = self.object_word(current, field).wrapping_add(xz_offset);
                    self.set_object_word(current, field, value);
                }
                let y_index = current.wrapping_add(c4).wrapping_add(7).wrapping_mul(4) & 0x003E;
                let y_offset = self.memory.read_long_word(0x7FC306 + u32::from(y_index)) as i16;
                let value = self.object_word(current, FIELD_Y).wrapping_add(y_offset);
                self.set_object_word(current, FIELD_Y, value);
            }
            Sf2PathOperation::FreeObjectAuxiliaryAndResetD742 => {
                let current = self.current_object()?;
                let block = self.memory.read_word(current.wrapping_add(0x1CE0));
                free_auxiliary(&mut self.memory, current, block);
                self.memory.write_word(current.wrapping_add(0x1CE0), 0);
                self.memory.write_byte(0xD742, 1);
            }
            Sf2PathOperation::ResetSelectedAuxiliaryMotion => {
                let selected = self.memory.read_word(SELECTED_OBJECT);
                let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
                let flags_address = 0x6B77u16.wrapping_add(slot);
                let flags = self.memory.read_byte(flags_address) & !0x04;
                self.memory.write_byte(flags_address, flags);
                self.memory.write_byte(0x6A61u16.wrapping_add(slot), 0);
            }
            Sf2PathOperation::IncrementLinkedAuxiliaryCounter => {
                let current = self.current_object()?;
                let linked = self.memory.read_word(current.wrapping_add(0x06));
                let slot = self.memory.read_word(linked.wrapping_add(FIELD_PATH));
                let address = 0x6C03u16.wrapping_add(slot);
                self.memory
                    .write_byte(address, self.memory.read_byte(address).wrapping_add(1));
            }
            Sf2PathOperation::DecrementLinkedAuxiliaryCounter => {
                let current = self.current_object()?;
                let linked = self.memory.read_word(current.wrapping_add(0x06));
                let slot = self.memory.read_word(linked.wrapping_add(FIELD_PATH));
                let address = 0x6C03u16.wrapping_add(slot);
                let counter = self.memory.read_byte(address);
                if counter != 0 {
                    self.memory.write_byte(address, counter - 1);
                }
            }
            Sf2PathOperation::SelectCurrentAsRotationTarget => {
                let current = self.current_object()?;
                self.memory.write_word(0x1DFF, current);
            }
            Sf2PathOperation::ClearSelectedAuxiliaryFlag01 => {
                let selected = self.memory.read_word(SELECTED_OBJECT);
                let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
                let address = 0x6B77u16.wrapping_add(slot);
                self.memory
                    .write_byte(address, self.memory.read_byte(address) & !0x01);
            }
            Sf2PathOperation::SetSelectedSlotLowNibble1 => {
                let selected = self.memory.read_word(SELECTED_OBJECT);
                let slot = self.memory.read_word(selected.wrapping_add(FIELD_PATH));
                let address = 0x6AA1u16.wrapping_add(slot);
                let value = self.memory.read_byte(address);
                self.memory.write_byte(address, (value & 0xF0) | 1);
            }
            Sf2PathOperation::ChaseObjectPositionTowardCurrent(target) => {
                let current = self.current_object()?;
                for field in [FIELD_X, FIELD_Y, FIELD_Z] {
                    let from = self.object_word(target, field);
                    let destination = self.object_word(current, field);
                    let value = Self::chase_word(from, destination, 3, 8);
                    self.set_object_word(target, field, value);
                }
            }
            Sf2PathOperation::CopySelectedRotation => {
                let current = self.current_object()?;
                let selected = self.memory.read_word(SELECTED_OBJECT);
                for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
                    let value = self.memory.read_byte(selected.wrapping_add(field));
                    self.memory.write_byte(current.wrapping_add(field), value);
                }
            }
            Sf2PathOperation::CopyPositionToObject(target) => {
                let current = self.current_object()?;
                for field in [FIELD_X, FIELD_Y, FIELD_Z] {
                    let value = self.memory.read_word(current.wrapping_add(field));
                    self.memory.write_word(target.wrapping_add(field), value);
                }
            }
            Sf2PathOperation::CopyRotationToObjectFixed(target) => {
                let current = self.current_object()?;
                for field in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z] {
                    let angle = self.memory.read_byte(current.wrapping_add(field));
                    self.memory
                        .write_word(target.wrapping_add(field), u16::from(angle) << 8);
                }
            }
            Sf2PathOperation::PopPathStackPair => {
                let current = self.current_object()?;
                for _ in 0..2 {
                    if let Some(value) = pop_path_stack(&mut self.memory, current) {
                        self.memory.write_word(0x16B1, value);
                    }
                }
            }
            Sf2PathOperation::ConfigurePlayerAuxiliary(value) => {
                self.configure_pilot_auxiliary(value, PilotAuxiliaryMode::FullControl)?;
            }
            Sf2PathOperation::SetObjectRotationTowardTarget { object, shift } => {
                let target = self.memory.read_word(0x1DFF);
                let (dx, dy, dz) = self.object_delta(object, target);
                let distance = sf2_xz_angle_distance(dx, dz) as u16;
                let pitch = (self.sf2_atan16(dy as u16, distance).wrapping_neg() as i16)
                    >> u32::from(shift & 7);
                let yaw = self.sf2_atan16(dx as u16, dz as u16) as i16;
                self.set_object_word(object, FIELD_ROT_X, pitch);
                self.set_object_word(object, FIELD_ROT_Y, yaw);
                let roll = self.object_word(object, FIELD_ROT_Z);
                self.set_object_word(object, FIELD_ROT_Z, Self::chase_word(roll, 0, 2, 4));
            }
            Sf2PathOperation::ChaseObjectRotationTowardTarget { object, shift } => {
                let target = self.memory.read_word(0x1DFF);
                let (dx, dy, dz) = self.object_delta(object, target);
                let distance = sf2_xz_angle_distance(dx, dz) as u16;
                let pitch = (self.sf2_atan16(dy as u16, distance).wrapping_neg() as i16)
                    >> u32::from(shift & 7);
                let yaw = self.sf2_atan16(dx as u16, dz as u16) as i16;
                for (field, destination) in
                    [(FIELD_ROT_X, pitch), (FIELD_ROT_Y, yaw), (FIELD_ROT_Z, 0)]
                {
                    let from = self.object_word(object, field);
                    self.set_object_word(object, field, Self::chase_word(from, destination, 2, 4));
                }
            }
            Sf2PathOperation::RefreshOwnedPlayerAuxiliaryOrigin => {
                let current = self.current_object()?;
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    if self.memory.read_word(0x6A98u16.wrapping_add(slot)) == current {
                        for (source, target) in [
                            (FIELD_X, 0x6A92u16),
                            (FIELD_Y, 0x6A94u16),
                            (FIELD_Z, 0x6A96u16),
                        ] {
                            let value = self.memory.read_word(current + source);
                            self.memory.write_word(target.wrapping_add(slot), value);
                        }
                    }
                }
            }
            Sf2PathOperation::InstallStrategyAndStop { strategy, state } => {
                let current = self.current_object()?;
                self.memory.write_word(current + FIELD_STRATEGY, strategy);
                self.memory.write_byte(current + FIELD_STRATEGY + 2, state);
                self.memory.write_byte(current.wrapping_add(0x1CC7), 0);
                self.memory.write_word(current + FIELD_PATH, 0);
            }
            Sf2PathOperation::CaptureSelectedAuxiliaryMotion => {
                self.capture_selected_auxiliary_motion()?;
            }
            Sf2PathOperation::LinkSpawnedObjectToCurrent => {
                let current = self.current_object()?;
                let spawned = self.memory.read_word(LAST_SPAWNED_OBJECT);
                self.memory
                    .write_word(spawned.wrapping_add(SPAWNED_OBJECT_OWNER_FIELD), current);
            }
            Sf2PathOperation::EaseFixedPlayerYaw => self.ease_fixed_player_yaw(),
            Sf2PathOperation::ConfigureRandomizedObjectMotion => {
                self.configure_randomized_object_motion()?;
            }
            Sf2PathOperation::ChaseYawOppositeFixedPlayer => {
                self.chase_yaw_opposite_fixed_player()?;
            }
            Sf2PathOperation::AccumulatePlayerAuxiliaryMotion => {
                self.accumulate_player_auxiliary_motion()?;
            }
            Sf2PathOperation::InitializePlayerAuxiliaryCharge => {
                self.initialize_player_auxiliary_charge();
            }
            Sf2PathOperation::LinkSelectedObjectTransform => {
                self.link_selected_object_transform()?;
            }
            Sf2PathOperation::RefreshPlayerAuxiliaryMode => {
                self.refresh_player_auxiliary_mode()?;
            }
            Sf2PathOperation::EnablePlayerAuxiliaryControl => {
                self.enable_player_auxiliary_control();
            }
            Sf2PathOperation::ChaseCurrentRelativeOffsets => {
                self.chase_current_relative_offsets()?;
            }
            Sf2PathOperation::AdvanceCurrentRelativeOffsets => {
                self.advance_current_relative_offsets()?;
            }
            Sf2PathOperation::ChaseCurrentRelativePose => {
                self.chase_current_relative_pose()?;
            }
            Sf2PathOperation::UpdateConditionalObjectPhase => {
                self.update_conditional_object_phase()?;
            }
            Sf2PathOperation::InitializePlayerRelativeMotion => {
                self.initialize_player_relative_motion()?;
            }
            Sf2PathOperation::InitializeSpawnedObjectMotion => {
                self.initialize_spawned_object_motion()?;
            }
            Sf2PathOperation::InitializeLaunchedExternalObject => {
                self.initialize_launched_external_object()?;
            }
            Sf2PathOperation::SpawnPlayerLinkedObject => {
                self.spawn_player_linked_object()?;
            }
            Sf2PathOperation::ResetPlayerAuxiliaryTarget => {
                self.reset_player_auxiliary_target()?;
            }
            Sf2PathOperation::ApplyCurrentHealthDecay => {
                self.apply_current_health_decay()?;
            }
            Sf2PathOperation::SeparateYawTargets => self.separate_yaw_targets()?,
            Sf2PathOperation::AdvanceVerticalOscillation => {
                self.advance_vertical_oscillation()?;
            }
        }
        Ok(())
    }

    fn evaluate_path_condition(
        &mut self,
        condition: Sf2PathCondition,
    ) -> Result<bool, Self::Error> {
        let current = self.current_object()?;
        Ok(match condition {
            Sf2PathCondition::HitGround { offset } => {
                self.object_word(current, FIELD_Y)
                    .wrapping_add(offset as i16)
                    < 0
            }
            Sf2PathCondition::ProjectedSelectedPointNegative => self
                .selected_object()
                .map(|selected| {
                    self.object_word(selected, FIELD_Z) < self.object_word(current, FIELD_Z)
                })
                .unwrap_or(false),
            Sf2PathCondition::SelectedLeftOfObject => self
                .selected_object()
                .map(|selected| {
                    self.object_word(selected, FIELD_Y) < self.object_word(current, FIELD_Y)
                })
                .unwrap_or(false),
            Sf2PathCondition::ProjectedSelectedForwardPointNegative => self
                .selected_object()
                .map(|selected| {
                    self.object_word(selected, FIELD_Z).wrapping_add(127)
                        < self.object_word(current, FIELD_Z)
                })
                .unwrap_or(false),
            Sf2PathCondition::SelectedBelowObject => self
                .selected_object()
                .map(|selected| {
                    self.object_word(selected, FIELD_Y) >= self.object_word(current, FIELD_Y)
                })
                .unwrap_or(false),
            Sf2PathCondition::SelectedOrCurrentAuxState => {
                let selected = self.selected_aux_flags().unwrap_or(0);
                let current_aux = self.memory.read_byte(current.wrapping_add(0x6B77));
                selected & 0xC0 != 0 || current_aux & 0x20 != 0
            }
            Sf2PathCondition::SelectedAuxiliaryMapCellOccupied => {
                let Some(selected) = self.selected_object() else {
                    return Ok(false);
                };
                let slot = self.memory.read_word(selected + FIELD_PATH);
                if self.memory.read_byte(0x6BEBu16.wrapping_add(slot)) & 0x80 != 0 {
                    false
                } else {
                    let x = self.memory.read_word(current + FIELD_X);
                    let z = self.memory.read_word(current + FIELD_Z);
                    let horizontal = (x.swap_bytes() >> 1) & 0x007F;
                    let mask_index = u32::from((horizontal & 7) << 1);
                    let mask = self.memory.read_long_word(0x00_B063 + mask_index) as u8;
                    let row = (z & 0xFE00).swap_bytes().wrapping_shl(3);
                    let cell = row.wrapping_add(horizontal >> 3);
                    self.memory.read_byte(0xCF36u16.wrapping_add(cell)) & mask != 0
                }
            }
            Sf2PathCondition::SelectedAuxiliaryFlag04Clear => {
                let Some(selected) = self.selected_object() else {
                    return Ok(false);
                };
                let slot = self.memory.read_word(selected + FIELD_PATH);
                self.memory.read_byte(0x6B77u16.wrapping_add(slot)) & 0x04 == 0
            }
            Sf2PathCondition::PlayerOneFlag25Bit20 => {
                let player = self.memory.read_word(PLAYER_ONE);
                self.memory
                    .read_byte(player.wrapping_add(OBJECT_FLAG_25_FIELD))
                    & PLAYER_ONE_TRANSFORM_LOCKED_FLAG
                    != 0
            }
        })
    }

    fn classify_path_contact(&mut self) -> Result<Option<PathContactClass>, Self::Error> {
        self.classify_path_contact_native_outer()
    }

    fn refresh_collision_target(&mut self) -> Result<(), Self::Error> {
        self.refresh_collision_projection()
    }

    fn cancel_trigger(&mut self, path: PathAddress) -> Result<(), Self::Error> {
        if self.memory.read_word(0xD777) == path.offset {
            self.memory.write_word(0xD777, 0);
        }
        Ok(())
    }

    fn force_trigger_path(&mut self, path: PathAddress) -> Result<(), Self::Error> {
        self.memory.write_word(0xD777, path.offset);
        Ok(())
    }

    fn update_player_target(&mut self, update: PlayerTargetUpdate) -> Result<(), Self::Error> {
        let current = self.current_object()?;

        // Both retail path wrappers suspend the coprocessor interrupt sources
        // before entering their shared player-target implementation.
        self.suspend_coprocessor_interrupts();

        let mode = match update {
            PlayerTargetUpdate::FlagLinked => 0,
            PlayerTargetUpdate::Flag08 => 8,
        };
        self.update_player_target_retail(current, mode);

        if update == PlayerTargetUpdate::FlagLinked {
            let linked = self.memory.read_word(current.wrapping_add(0x1CE6));
            if linked != 0 {
                let value = self.memory.read_byte(linked.wrapping_add(0x12)) | 0x20;
                self.memory.write_byte(linked.wrapping_add(0x12), value);
            }
        }
        Ok(())
    }

    fn queue_selected_marker(
        &mut self,
        value: u8,
        class: SelectedMarkerClass,
    ) -> Result<(), Self::Error> {
        if class == SelectedMarkerClass::Direct {
            let selected = self.memory.read_word(SELECTED_OBJECT);
            self.enqueue_event(u16::from(value), selected);
        }
        self.map_markers.push(MapMarker {
            kind: value,
            table_index: match class {
                SelectedMarkerClass::Direct => 0,
                SelectedMarkerClass::Class1 => 1,
                SelectedMarkerClass::Class2 => 2,
            },
        });
        Ok(())
    }

    fn spawn_linked_object_effects(&mut self) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        self.memory.write_word(0x04, current);
        self.memory.write_byte(0xE4, 0);

        // The retail helper disables both graphics-coprocessor IRQ sources
        // before walking the linked launch list and mirrors the masked value
        // to the hardware control port. The compatibility host does not need
        // that port write, but the game-owned shadow byte is observable.
        self.suspend_coprocessor_interrupts();

        if self.memory.read_byte(current + 0x25) & 0x10 == 0 {
            return Ok(());
        }
        let mut node = self.memory.read_word(current + 0x1E);
        while node != 0 {
            let linked = self.memory.read_word(node.wrapping_add(4));
            let eligible = self.memory.read_byte(linked.wrapping_add(0x31)) & 0x08 != 0
                && self.memory.read_byte(linked.wrapping_add(0x21)) & 0x01 == 0;
            if eligible {
                // The source routine reuses its launch-origin scratch after
                // the first list entry. With a zero local offset, later list
                // entries therefore launch from the world origin. Preserve
                // that observable behavior without modeling processor state.
                let source = self.memory.read_word(0x04);
                let value = self.memory.read_byte(linked + 0x21) | 0x01;
                self.memory.write_byte(linked + 0x21, value);

                let mut pitch = self.memory.read_byte(linked + FIELD_ROT_X).wrapping_neg();
                let mut yaw = self
                    .memory
                    .read_byte(linked + FIELD_ROT_Y)
                    .wrapping_add(0x80)
                    .wrapping_sub(self.memory.read_byte(source + FIELD_ROT_Y));
                let source_is_player = source == self.memory.read_word(PLAYER_ONE)
                    || source == self.memory.read_word(PLAYER_TWO);
                let stable_player_launch = source_is_player && {
                    let slot = self.memory.read_word(source + FIELD_PATH);
                    self.memory.read_byte(0x6C02u16.wrapping_add(slot)) & 0x40 == 0
                };
                if !stable_player_launch {
                    pitch = pitch
                        .wrapping_add(self.random_byte() & 0x3F)
                        .wrapping_add(0xE0);
                    yaw = yaw
                        .wrapping_add(self.random_byte() & 0x3F)
                        .wrapping_add(0xE0);
                }

                self.memory.write_byte(0x14B7, pitch);
                self.memory.write_byte(0x14B6, yaw);
                for address in [0x14B8u16, 0x14B9, 0x14B0, 0x14B2, 0x14B4] {
                    self.memory.write_byte(address, 0);
                }

                let spawned = self
                    .spawn_path_weapon(source, 0x02)
                    .unwrap_or_else(|| self.memory.read_word(0x14D6));
                // `$03:AB1D` uses direct-page $E4 as its transformed Z offset.
                // The handler set that launch offset to zero immediately above.
                self.memory.write_byte(0xE4, 0);
                self.memory.write_word(0x04, 0);
                let shape = find_auxiliary_type(&self.memory, linked, 0x0A)
                    .map(|entry| read_auxiliary_word(&self.memory, entry.wrapping_add(1)))
                    .unwrap_or_else(|| self.memory.read_word(linked + FIELD_SHAPE));
                self.memory
                    .write_word(spawned.wrapping_add(FIELD_SHAPE), shape);
                self.memory.write_word(
                    spawned.wrapping_add(0x1CCD),
                    self.memory.read_word(linked.wrapping_add(0x1CCD)),
                );
                for field in [FIELD_X, FIELD_Y, FIELD_Z] {
                    self.memory.write_word(
                        spawned.wrapping_add(field),
                        self.memory.read_word(linked.wrapping_add(field)),
                    );
                }
                if self.memory.read_byte(linked + 0x20) & 0x20 != 0 {
                    let value = self.memory.read_byte(spawned.wrapping_add(0x20)) | 0x20;
                    self.memory.write_byte(spawned.wrapping_add(0x20), value);
                    for extension in [0x1CC8u16, 0x1CDA] {
                        self.memory.write_byte(
                            spawned.wrapping_add(extension),
                            self.memory.read_byte(linked.wrapping_add(extension)),
                        );
                    }
                    self.memory.write_byte(linked + 0x18, 0x3C);
                }
                let count = self.memory.read_byte(0xE4).wrapping_add(1);
                self.memory.write_byte(0xE4, count);

                if self.memory.read_byte(0x1AA6) & 0x02 == 0 {
                    break;
                }
            }
            node = self.memory.read_word(node);
        }
        self.memory.write_word(CURRENT_OBJECT, current);
        Ok(())
    }

    fn random_byte(&mut self) -> Result<u8, Self::Error> {
        Ok(Game::random_byte(self))
    }

    fn do_queue(&mut self, queue: u8) -> Result<(), Self::Error> {
        self.memory.write_byte(0xD786, queue);
        Ok(())
    }

    fn set_trail(&mut self, trail: u8) -> Result<(), Self::Error> {
        self.write_object_extension_byte(0x1CCC, trail)
    }

    fn regenerate_velocity_vectors(&mut self) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        let speed = i16::from(self.memory.read_byte(current + 0x18));
        let yaw = self.memory.read_byte(current + FIELD_ROT_Y);
        self.set_object_word(
            current,
            0x1CD0,
            (i16::from(sf_core::snes_trig::SINTAB[yaw as usize]) * speed) / 64,
        );
        self.set_object_word(
            current,
            0x1CD2,
            (i16::from(sf_core::snes_trig::COSTAB[yaw as usize]) * speed) / 64,
        );
        Ok(())
    }

    fn set_sprite(&mut self, x: u8, y: u8) -> Result<(), Self::Error> {
        self.memory.write_byte(0x1D7A, x);
        self.memory.write_byte(0x1D7B, y);
        Ok(())
    }

    fn quick_spawn(
        &mut self,
        shape: u16,
        path: PathAddress,
        hit_points: u8,
        attack_points: u8,
    ) -> Result<(), Self::Error> {
        self.spawn_full(ObjectSpawn {
            shape,
            path,
            rotation: [0; 3],
            hit_points,
            attack_points,
            offset: [0; 3],
        })?;
        Ok(())
    }

    fn spawn_child(&mut self, spawn: ChildSpawn) -> Result<(), Self::Error> {
        let current = self.current_object()?;
        let mother = if self.memory.read_byte(current + 0x23) & 0x04 != 0 {
            self.memory.read_word(current + 0x06)
        } else {
            current
        };
        let object = self.spawn_full(ObjectSpawn {
            shape: spawn.shape,
            path: spawn.path,
            rotation: spawn.rotation,
            hit_points: spawn.hit_points,
            attack_points: spawn.attack_points,
            offset: spawn.offset,
        })?;
        self.memory.write_word(object + 0x06, mother);
        self.memory.write_byte(object + 0x13, spawn.child_number);
        self.memory.write_word(object + 0x29, 0);
        if mother != 0 {
            let mut tail = mother;
            while self.memory.read_word(tail + 0x29) != 0 {
                tail = self.memory.read_word(tail + 0x29);
            }
            self.memory.write_word(tail + 0x29, object);
            let flags = self.memory.read_byte(mother + 0x23) | 0x10;
            self.memory.write_byte(mother + 0x23, flags);
        }
        let flags_23 = self.memory.read_byte(object + 0x23) | 0x04;
        let flags_25 = self.memory.read_byte(object + 0x25) | 0x01;
        self.memory.write_byte(object + 0x23, flags_23);
        self.memory.write_byte(object + 0x25, flags_25);
        self.memory.write_word(object.wrapping_add(0x1CD8), mother);
        for (offset, value) in [
            (0x1CCF, spawn.offset[0]),
            (0x1CD1, spawn.offset[1]),
            (0x1CD3, spawn.offset[2]),
        ] {
            self.memory
                .write_word(object.wrapping_add(offset), value as u16);
        }
        for (offset, value) in [
            (0x1CD5, spawn.rotation[0]),
            (0x1CD6, spawn.rotation[1]),
            (0x1CD7, spawn.rotation[2]),
        ] {
            self.memory.write_byte(object.wrapping_add(offset), value);
        }
        Ok(())
    }

    fn remove_child(&mut self, child_number: u8) -> Result<(), Self::Error> {
        if !self.child_is_dead(child_number)? {
            if let Some(child) = self.selected_object() {
                let flags = self.memory.read_byte(child + 0x25) | 0x08;
                self.memory.write_byte(child + 0x25, flags);
            }
        }
        Ok(())
    }

    fn start_message(&mut self, message: u8) -> Result<(), Self::Error> {
        self.messages.push(message);
        Ok(())
    }

    fn schedule_trigger(&mut self, trigger: PathTrigger) -> Result<(), Self::Error> {
        self.memory.write_word(0xD777, trigger.path.offset);
        self.memory.write_byte(0xD779, trigger.delay);
        self.memory.write_byte(0xD77A, trigger.trigger);
        Ok(())
    }

    fn push_path_value(&mut self, value: u16) -> Result<(), Self::Error> {
        let object = self.current_object()?;
        push_path_stack(&mut self.memory, object, value)
            .then_some(())
            .ok_or(Error::AuxiliaryHeapExhausted)
    }

    fn pop_path_value(&mut self) -> Result<Option<u16>, Self::Error> {
        let object = self.current_object()?;
        Ok(pop_path_stack(&mut self.memory, object))
    }

    fn transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<(), Self::Error> {
        match transition {
            ContextTransition::Unbecome => {
                let current = self.current_object()?;
                let parent = self.memory.read_word(0xD76D);
                if object_index(parent).is_some() {
                    self.memory
                        .write_word(current + FIELD_PATH, self.memory.read_word(0xD76F));
                    self.memory
                        .write_word(parent + FIELD_PATH, resume_at.offset);
                    self.memory.write_word(CURRENT_OBJECT, parent);
                }
            }
            ContextTransition::Become => {
                let selected = self.memory.read_word(0xD771);
                if object_index(selected).is_some() {
                    let current = self.current_object()?;
                    self.memory.write_word(0xD76D, current);
                    self.memory
                        .write_word(0xD76F, self.memory.read_word(selected + FIELD_PATH));
                    self.memory
                        .write_word(current + FIELD_PATH, resume_at.offset);
                    self.memory
                        .write_word(selected + FIELD_PATH, resume_at.offset);
                    self.memory.write_word(CURRENT_OBJECT, selected);
                }
            }
            transition => {
                self.try_transition_context(transition, resume_at)?;
            }
        }
        Ok(())
    }
}
