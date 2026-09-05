//! SF2's authored clipping planes, independent of the source machine.
//!
//! The sweep shapes define two local points. Transforming their difference
//! produces a plane normal; the transformed origin and object translation
//! determine its distance. Integer scene endpoints retain per-product
//! rounding and wrapping. An HD scene can interpolate presentation separately.

use sf2_data::shape_data::{ShapeClipPlane, ShapeVertex};

const FRACTION_BITS: u32 = 15;
const NORMAL_SCALE: i16 = 8;

fn product(left: i16, right: i16) -> i16 {
    ((i32::from(left) * i32::from(right)) >> FRACTION_BITS) as i16
}

fn dot(left: ShapeVertex, right: ShapeVertex) -> i16 {
    product(left.x, right.x)
        .wrapping_add(product(left.y, right.y))
        .wrapping_add(product(left.z, right.z))
}

fn add(left: ShapeVertex, right: ShapeVertex) -> ShapeVertex {
    ShapeVertex {
        x: left.x.wrapping_add(right.x),
        y: left.y.wrapping_add(right.y),
        z: left.z.wrapping_add(right.z),
    }
}

/// Three row vectors in signed Q15, without translation. Callers supply the
/// same object-to-view transform as the geometry being clipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneTransform {
    pub x: ShapeVertex,
    pub y: ShapeVertex,
    pub z: ShapeVertex,
}

impl PlaneTransform {
    fn transform(self, point: ShapeVertex) -> ShapeVertex {
        ShapeVertex {
            x: dot(self.x, point),
            y: dot(self.y, point),
            z: dot(self.z, point),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipPlane {
    pub normal: ShapeVertex,
    pub distance: i16,
}

impl ClipPlane {
    /// Reconstruct one authored plane; do not normalize or negate a paired
    /// plane, since the opposing directions have distinct integer rounding.
    pub fn from_definition(
        definition: ShapeClipPlane,
        transform: PlaneTransform,
        translation: ShapeVertex,
    ) -> Self {
        let origin = transform.transform(definition.origin);
        let endpoint = transform.transform(definition.direction_point);
        let normal = ShapeVertex {
            x: endpoint.x.wrapping_sub(origin.x).wrapping_mul(NORMAL_SCALE),
            y: endpoint.y.wrapping_sub(origin.y).wrapping_mul(NORMAL_SCALE),
            z: endpoint.z.wrapping_sub(origin.z).wrapping_mul(NORMAL_SCALE),
        };
        Self {
            normal,
            distance: dot(normal, add(origin, translation)),
        }
    }

    /// Rebase a plane for a mesh whose rotated vertices have not yet had
    /// their object translation added. Rebasing precedes vertex products.
    pub fn relative_to(self, translation: ShapeVertex) -> Self {
        Self {
            normal: self.normal,
            distance: self.distance.wrapping_sub(dot(self.normal, translation)),
        }
    }

    /// Nonnegative vertices survive. Round each product independently;
    /// rounding a summed wide dot product changes near-plane classification.
    pub fn signed_distance(self, rotated_vertex: ShapeVertex) -> i16 {
        dot(self.normal, rotated_vertex).wrapping_sub(self.distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: ShapeVertex = ShapeVertex { x: 0, y: 0, z: 0 };
    const ALMOST_ONE: i16 = 32_766;
    const IDENTITY: PlaneTransform = PlaneTransform {
        x: ShapeVertex {
            x: ALMOST_ONE,
            y: 0,
            z: 0,
        },
        y: ShapeVertex {
            x: 0,
            y: ALMOST_ONE,
            z: 0,
        },
        z: ShapeVertex {
            x: 0,
            y: 0,
            z: ALMOST_ONE,
        },
    };

    #[test]
    fn paired_planes_are_not_rounded_negations() {
        let definitions = sf2_data::shape_data::SHAPE_DATA[48].clipping_planes;
        let first = ClipPlane::from_definition(definitions[0], IDENTITY, ORIGIN);
        let second = ClipPlane::from_definition(definitions[1], IDENTITY, ORIGIN);
        assert_eq!(first.normal.y, -32_760);
        assert_eq!(second.normal.y, 32_752);
        assert_eq!(first.distance, 0);
        assert_eq!(second.distance, 0);
        assert!(first.signed_distance(ShapeVertex { y: -10, ..ORIGIN }) > 0);
        assert!(second.signed_distance(ShapeVertex { y: -10, ..ORIGIN }) < 0);
    }

    #[test]
    fn translated_plane_rebases_before_vertex_classification() {
        let definition = sf2_data::shape_data::SHAPE_DATA[48].clipping_planes[0];
        let translation = ShapeVertex { y: 123, ..ORIGIN };
        let plane = ClipPlane::from_definition(definition, IDENTITY, translation);
        assert_eq!(plane.signed_distance(translation), 0);
        let relative = plane.relative_to(translation);
        assert_eq!(relative.distance, 0);
        assert_eq!(relative.signed_distance(ORIGIN), 0);
    }
}
