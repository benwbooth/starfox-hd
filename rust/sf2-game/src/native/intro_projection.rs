//! Source-resolution SF2 mesh projection, before visibility and BSP traversal.
//! Inputs are already rotated into camera axes. Object translation is added
//! with word wrapping; no floating-point HD coordinates enter this stage.

use super::intro_visibility::ProjectedShapePoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionViewport {
    pub center: [i16; 2],
    pub left: i16,
    pub right: i16,
    pub top: i16,
    pub bottom: i16,
}

/// Project one camera-space point using the original mesh path. The two depth
/// paths deliberately retain different rounding and screen-edge comparisons.
pub fn project_point([x, y, mut z]: [i16; 3], viewport: ProjectionViewport) -> ProjectedShapePoint {
    // Original BPL tests the wrapped result, not an overflow-aware comparison.
    if 0x3000i16.wrapping_sub(z) < 0 {
        z = 0x2FFF;
    }
    if 256i16.wrapping_sub(z) >= 0 {
        return project_near([x, y, z], viewport);
    }
    let depth = (z as u16) & !1;
    // Exact integer formula for ROM $19:BAB8, not a floating reciprocal.
    let reciprocal = (32767u32 * 256 / u32::from(depth)) as i32;
    let project = |coordinate: i16, center: i16| {
        ((i32::from(coordinate) * reciprocal) >> 16).wrapping_add(i32::from(center)) as i16
    };
    let x = project(x, viewport.center[0]);
    let y = project(y, viewport.center[1]);
    let mut outside = 0;
    if x < 0 {
        outside |= 4;
    } else if x.wrapping_sub(viewport.right) >= 0 {
        outside |= 8;
    }
    if y < 0 {
        outside |= 1;
    } else if y.wrapping_sub(viewport.bottom) >= 0 {
        outside |= 2;
    }
    projected(x, y, outside)
}

fn projected(x: i16, y: i16, outside: u16) -> ProjectedShapePoint {
    ProjectedShapePoint {
        x,
        y,
        outcode: ((outside ^ 31) << 8) | outside,
    }
}

fn project_near([x, y, z]: [i16; 3], viewport: ProjectionViewport) -> ProjectedShapePoint {
    let behind = z < 0;
    let depth = z.unsigned_abs().max(1);
    let absolute_x = x.unsigned_abs();
    let absolute_y = y.unsigned_abs();
    let (projected_x, projected_y) = if absolute_x >= absolute_y {
        project_magnitudes(absolute_x, absolute_y, depth)
    } else {
        let (major, minor) = project_magnitudes(absolute_y, absolute_x, depth);
        (minor, major)
    };
    let restore = |magnitude: u16, negative: bool, center: i16| {
        let magnitude = magnitude as i16;
        (if negative {
            magnitude.wrapping_neg()
        } else {
            magnitude
        })
        .wrapping_add(center)
    };
    let x = restore(projected_x, (x < 0) ^ behind, viewport.center[0]);
    let y = restore(projected_y, (y < 0) ^ behind, viewport.center[1]);
    let mut horizontal = if x <= viewport.left {
        1
    } else if x > viewport.right {
        2
    } else {
        0
    };
    let mut vertical = if y <= viewport.top {
        1
    } else if y > viewport.bottom {
        2
    } else {
        0
    };
    if behind {
        if horizontal != 0 {
            horizontal ^= 3;
        }
        if vertical != 0 {
            vertical ^= 3;
        }
    }
    projected(
        x,
        y,
        horizontal * 4 + vertical + if behind { 16 } else { 0 },
    )
}

fn project_magnitudes(major: u16, minor: u16, depth: u16) -> (u16, u16) {
    // ASR is intentional even for the wrapped magnitude of i16::MIN.
    let mut major = ((major as i16) >> 1) as u16;
    let mut minor = ((minor as i16) >> 1) as u16;
    if major >> 8 < depth {
        major = divide_projection(u32::from(major) << 8, depth);
        minor = divide_projection(u32::from(minor) << 8, depth);
        if major <= 0x3FFF {
            return (major, minor);
        }
    }
    minor = divide_projection(u32::from(minor) * 0x3FFF, major);
    (0x3FFF, minor)
}

fn divide_projection(mut numerator: u32, mut denominator: u16) -> u16 {
    // The source's restoring divider normalizes high-bit operands first.
    if numerator & 0x8000_0000 != 0 || denominator & 0x8000 != 0 {
        numerator >>= 1;
        denominator >>= 1;
    }
    (numerator / u32::from(denominator)) as u16
}

/// Source rotated-point list plus camera-axis object translation. An empty
/// native list yields no points; it does not encode the machine's zero counter.
pub fn project_points(
    rotated_points: &[[i16; 3]],
    translation: [i16; 3],
    viewport: ProjectionViewport,
) -> Vec<ProjectedShapePoint> {
    rotated_points
        .iter()
        .map(|point| {
            project_point(
                std::array::from_fn(|axis| point[axis].wrapping_add(translation[axis])),
                viewport,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: ProjectionViewport = ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    };

    #[test]
    fn far_path_keeps_high_word_rounding_and_near_path_keeps_edge_ties() {
        assert_eq!(
            project_point([256, 0, 512], VIEWPORT),
            projected(175, 96, 0)
        );
        assert_eq!(project_point([-224, 0, 256], VIEWPORT), projected(0, 96, 4));
        assert_eq!(
            project_point([224, 0, 256], VIEWPORT),
            projected(224, 96, 0)
        );
        assert_eq!(
            project_point([0, -192, 256], VIEWPORT),
            projected(112, 0, 1)
        );
        assert_eq!(
            project_point([0, 192, 256], VIEWPORT),
            projected(112, 192, 0)
        );
    }

    #[test]
    fn zero_and_wrapped_negative_depths_keep_source_behavior() {
        assert_eq!(project_point([0, 0, 0], VIEWPORT), projected(112, 96, 0));
        // BPL, not BGE: this signed subtraction wraps and takes the far clamp.
        assert_eq!(
            project_point([0, 0, i16::MIN], VIEWPORT),
            projected(112, 96, 0)
        );
        assert_eq!(
            project_point([0, 0, -20479], VIEWPORT),
            projected(112, 96, 16)
        );
        assert_eq!(
            project_point([i16::MIN, 0, 1], VIEWPORT),
            projected(-16271, 96, 4)
        );
    }

    #[test]
    fn near_saturation_preserves_dominant_axis_and_quantized_ratio() {
        assert_eq!(project_magnitudes(300, 199, 1), (0x3FFF, 0x2A3C));
        assert_eq!(project_magnitudes(32767, 16385, 1), (0x3FFF, 8192));
        assert_eq!(
            project_point([300, 199, 1], VIEWPORT),
            projected(16495, 10908, 10)
        );
    }

    #[test]
    fn object_translation_wraps_before_depth_selection() {
        let result = project_points(&[[32767, 0, 32767]], [1, 0, 1], VIEWPORT);
        assert_eq!(result, [project_point([i16::MIN, 0, i16::MIN], VIEWPORT)]);
        assert!(project_points(&[], [0; 3], VIEWPORT).is_empty());
    }
}
