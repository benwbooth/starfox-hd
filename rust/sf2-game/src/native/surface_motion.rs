//! Collision-constrained actor movement ($0D:B282..B6B7), including the
//! original bounded normal response, damping and footprint escape.

use super::collision_surface::{
    self, SurfaceFootprint, SurfaceGeometry, SurfaceQueryError, SurfaceSearch,
};
use super::{path_math, Angle, ObjectId, ObjectStore, Vector3};

#[cfg(test)]
#[path = "surface_motion_tests.rs"]
mod tests;

const DEAD_ZONE: u16 = 3;
const MOTION_SCALE: i16 = 8;
const AIR_DAMPING: i16 = 30_720;
const SURFACE_DAMPING: i16 = 8_192;
const NORMAL_RESPONSE_FACTOR: i16 = 16_384;
const ESCAPE_MARGIN: i16 = 6;
const LARGE_PENETRATION: u16 = 30;
const CONTACT_MARGIN: i16 = 3;
const WORD_BITS: u32 = 16;

/// Tilt inherited at this service boundary. It is not reconstructed from
/// the selected polygon normal. The first grounded response replaces it
/// with byte-aligned angles, whose low-byte handoff is zero on later probes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTilt {
    pub pitch: Angle,
    pub roll: Angle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMotionInputs {
    pub search: SurfaceSearch,
    pub strategy_tick: u8,
    /// None is the source's nonzero mode: no gravity update. Some applies
    /// gravity after integration and before the response's precision shift.
    pub gravity: Option<i16>,
    pub inherited_tilt: SurfaceTilt,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMotionResult {
    pub retried: bool,
    pub restored_horizontal_position: bool,
    pub obstructed: bool,
    /// The final source length uses one's-complement differences, not
    /// ordinary displacement. Preserve that observable measurement.
    pub travel_measure: u16,
    pub inherited_tilt: SurfaceTilt,
    /// Includes rejected candidates and every escape/restoration retry,
    /// not merely the final selected support.
    pub broad_candidate_seen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceMotionError {
    Query(SurfaceQueryError),
    MissingActor(ObjectId),
    InvalidPolygon,
    NonterminatingResponse,
}

impl From<SurfaceQueryError> for SurfaceMotionError {
    fn from(error: SurfaceQueryError) -> Self {
        Self::Query(error)
    }
}

fn high_product(a: i16, b: i16) -> i16 {
    ((i32::from(a) * i32::from(b)) >> WORD_BITS) as i16
}

/// `$01:FAC0/FAC6`: doubling wraps before the signed high product. A
/// widened multiply by fifteen-sixteenths/one-quarter is not equivalent.
pub fn damp_horizontal(value: i16, grounded: bool) -> i16 {
    let factor = if grounded {
        SURFACE_DAMPING
    } else {
        AIR_DAMPING
    };
    high_product(value.wrapping_mul(2), factor)
}

/// `$01:FAE4`: retain each doubled-word wrap and separately truncated
/// product, including the vertical one's-complement correction.
pub fn normal_response(velocity: Vector3, normal: Vector3) -> Vector3 {
    let dot = high_product(velocity.x.wrapping_mul(2), normal.x)
        .wrapping_add(high_product(velocity.y.wrapping_mul(2), normal.y))
        .wrapping_add(high_product(velocity.z.wrapping_mul(2), normal.z))
        .wrapping_mul(2);
    let axis = |velocity: i16, normal: i16, projection: i16, correction: i16| {
        let response = high_product(normal, projection);
        high_product(response.wrapping_mul(2), NORMAL_RESPONSE_FACTOR)
            .wrapping_mul(2)
            .wrapping_add(velocity)
            .wrapping_add(correction)
    };
    Vector3 {
        x: axis(velocity.x, normal.x, dot, 0),
        y: axis(
            velocity.y,
            normal.y,
            !dot,
            high_product(normal.y, -1).wrapping_mul(2),
        ),
        z: axis(velocity.z, normal.z, dot, 0),
    }
}

/// The polygon leaf compares perpendicular distance to infinite edges, not
/// closest points on segments. Ties retain the first authored edge.
pub fn polygon_escape(
    vertices: &[[i8; 2]],
    scale: u8,
    point: [i16; 2],
) -> Result<[i16; 2], SurfaceMotionError> {
    if vertices.len() < 2 {
        return Err(SurfaceMotionError::InvalidPolygon);
    }
    let scale_axis = |value: i8| {
        if u32::from(scale) >= WORD_BITS {
            0
        } else {
            i16::from(value).wrapping_shl(u32::from(scale))
        }
    };
    let mut distance = u16::MAX;
    let mut closest = [0_i16; 2];
    for edge in vertices.windows(2) {
        let [start_x, start_z] = edge[0].map(scale_axis);
        let [end_x, end_z] = edge[1].map(scale_axis);
        let normal = path_math::normalized_direction(Vector3 {
            x: end_z.wrapping_sub(start_z),
            y: 0,
            z: start_x.wrapping_sub(end_x),
        });
        let projection =
            high_product(end_x.wrapping_sub(point[0]).wrapping_mul(2), normal.x).wrapping_add(
                high_product(end_z.wrapping_sub(point[1]).wrapping_mul(2), normal.z),
            );
        let absolute = projection.unsigned_abs();
        if absolute < distance {
            distance = absolute;
            closest = [normal.x, normal.z];
        }
    }
    let amount = (distance as i16)
        .wrapping_add(ESCAPE_MARGIN)
        .wrapping_mul(2);
    Ok(closest.map(|axis| high_product(axis, amount)))
}

fn escape(geometry: SurfaceGeometry, position: Vector3) -> Result<Vector3, SurfaceMotionError> {
    let (x, z) = match geometry.footprint {
        SurfaceFootprint::Rectangle {
            negative_x,
            negative_z,
            positive_x,
            positive_z,
        } => {
            let mut distance = u16::MAX;
            let mut selected = (0, 0);
            for (value, is_z) in [
                (negative_x, false),
                (negative_z, true),
                (positive_x, false),
                (positive_z, true),
            ] {
                if value.unsigned_abs() < distance {
                    distance = value.unsigned_abs();
                    selected = if is_z { (0, value) } else { (value, 0) };
                }
            }
            selected
        }
        SurfaceFootprint::Polygon {
            polygon,
            origin,
            yaw,
        } => {
            let (x, z) = super::collision_math::local_probe(
                yaw,
                position.x.wrapping_sub(origin.x),
                position.z.wrapping_sub(origin.z),
            );
            let [x, z] = polygon_escape(polygon.vertices, polygon.scale, [x, z])?;
            super::collision_math::local_probe(
                Angle::from_units(yaw.units().wrapping_neg()),
                x.wrapping_neg(),
                z.wrapping_neg(),
            )
        }
    };
    Ok(Vector3 { x, y: 0, z })
}

fn dead_zone(value: i16) -> i16 {
    if (value as u16).wrapping_add(DEAD_ZONE) < DEAD_ZONE * 2 {
        0
    } else {
        value
    }
}

fn multiply(value: Vector3, factor: i16) -> Vector3 {
    Vector3 {
        x: value.x.wrapping_mul(factor),
        y: value.y.wrapping_mul(factor),
        z: value.z.wrapping_mul(factor),
    }
}

pub fn advance(
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: SurfaceMotionInputs,
) -> Result<SurfaceMotionResult, SurfaceMotionError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(SurfaceMotionError::MissingActor(owner))?;
    let previous_position = actor.base.position;
    let previous_contact = actor.extension.surface_contact;
    let velocity = actor.base.velocity;
    super::path_motion::integrate(
        &mut actor.base.position,
        Vector3 {
            x: dead_zone(velocity.x),
            y: dead_zone(velocity.y),
            z: dead_zone(velocity.z),
        },
    );
    super::path_motion::integrate(
        &mut actor.base.position,
        actor.extension.path_state.platform_carry.saved_position,
    );
    if let Some(gravity) = inputs.gravity {
        actor.base.velocity.y = actor.base.velocity.y.wrapping_add(gravity);
    }
    actor.base.velocity = multiply(actor.base.velocity, MOTION_SCALE);
    actor.extension.path_state.motion_delta =
        multiply(actor.extension.path_state.motion_delta, MOTION_SCALE);
    let mut result = SurfaceMotionResult {
        inherited_tilt: inputs.inherited_tilt,
        ..Default::default()
    };
    let mut attempted_positions = Vec::new();
    loop {
        let position = objects.get(owner).expect("live motion owner").base.position;
        // Normal source flow terminates after the escape and optional X/Z
        // restoration. Diagnose malformed geometry's repeated state without
        // pretending that it completed a collision response.
        let signature = (
            position,
            result.retried,
            result.restored_horizontal_position,
        );
        if attempted_positions.contains(&signature) {
            return Err(SurfaceMotionError::NonterminatingResponse);
        }
        attempted_positions.push(signature);
        let query = collision_surface::query_object_surface_geometry(
            objects,
            owner,
            inputs.strategy_tick,
            inputs.search,
        )?;
        result.broad_candidate_seen |= query.broad_candidate_seen;
        objects.get_mut(owner).unwrap().extension.surface_contact = query.surface.contact;
        if query.surface.contact.supporting_object.is_none() {
            // A missed query publishes the cleared tilt before either the
            // grounded or airborne branch; it does not retain caller input.
            result.inherited_tilt = SurfaceTilt::default();
        }
        let mut grounded = false;
        if query.surface.contact.supporting_object == previous_contact.supporting_object
            && query.surface.contact.group == previous_contact.group
        {
            if let Some(support) = query.surface.contact.supporting_object {
                let support_velocity = objects
                    .get(support)
                    .ok_or(SurfaceMotionError::MissingActor(support))?
                    .base
                    .velocity
                    .y;
                let candidate = position.y.wrapping_add(support_velocity);
                if candidate
                    .wrapping_add(CONTACT_MARGIN)
                    .wrapping_sub(query.surface.height)
                    >= 0
                {
                    objects.get_mut(owner).unwrap().base.position.y = candidate;
                    grounded = true;
                } else {
                    break;
                }
            }
        }
        if !grounded {
            if position
                .y
                .wrapping_add(CONTACT_MARGIN)
                .wrapping_sub(query.surface.height)
                < 0
            {
                break;
            }
            let depth = position.y.wrapping_sub(query.surface.height);
            if depth >= 0
                && depth as u16 >= LARGE_PENETRATION
                && !result.restored_horizontal_position
            {
                result.obstructed = true;
                if result.retried {
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.position.x = previous_position.x;
                    actor.base.position.z = previous_position.z;
                    result.restored_horizontal_position = true;
                } else {
                    let push = escape(query.geometry, position)?;
                    super::path_motion::integrate(
                        &mut objects.get_mut(owner).unwrap().base.position,
                        push,
                    );
                }
                result.retried = true;
                continue;
            }
        }
        let actor = objects.get_mut(owner).unwrap();
        actor.base.flags.standing_on_surface = true;
        let tilt = if query.surface.contact.supporting_object.is_some() {
            result.inherited_tilt
        } else {
            SurfaceTilt::default()
        };
        let matrix = sf_core::snes_trig::zxy_matrix_q15(
            tilt.pitch.units().wrapping_neg(),
            0,
            tilt.roll.units().wrapping_neg(),
        );
        let delta = actor.extension.path_state.motion_delta;
        let (x, y, z) = sf_core::snes_trig::matrix_rotate_q15(matrix, delta.x, 0, delta.z);
        actor.extension.path_state.platform_carry.saved_position = Vector3 {
            x: x / MOTION_SCALE,
            y: (y / MOTION_SCALE).wrapping_add(delta.y),
            z: z / MOTION_SCALE,
        };
        result.inherited_tilt = SurfaceTilt::default();
        actor.base.velocity = normal_response(actor.base.velocity, query.geometry.normal);
        actor.base.position.y = query.surface.height;
        actor.base.velocity.x = damp_horizontal(actor.base.velocity.x, true);
        actor.base.velocity.z = damp_horizontal(actor.base.velocity.z, true);
        return finish(actor, previous_position, result);
    }
    let actor = objects.get_mut(owner).expect("live airborne owner");
    actor.base.flags.standing_on_surface = false;
    let delta = actor.extension.path_state.motion_delta;
    actor.extension.path_state.platform_carry.saved_position = Vector3 {
        x: delta.x / MOTION_SCALE,
        y: delta.y,
        z: delta.z / MOTION_SCALE,
    };
    actor.base.velocity.x = damp_horizontal(actor.base.velocity.x, false);
    actor.base.velocity.z = damp_horizontal(actor.base.velocity.z, false);
    finish(actor, previous_position, result)
}

fn finish(
    actor: &mut super::Object,
    previous: Vector3,
    mut result: SurfaceMotionResult,
) -> Result<SurfaceMotionResult, SurfaceMotionError> {
    actor.extension.path_state.motion_delta = Vector3::default();
    actor.base.velocity = Vector3 {
        x: actor.base.velocity.x / MOTION_SCALE,
        y: actor.base.velocity.y / MOTION_SCALE,
        z: actor.base.velocity.z / MOTION_SCALE,
    };
    result.travel_measure = path_math::vector_length(Vector3 {
        x: actor.base.position.x.wrapping_add(!previous.x),
        y: actor.base.position.y.wrapping_add(!previous.y),
        z: actor.base.position.z.wrapping_add(!previous.z),
    });
    Ok(result)
}
