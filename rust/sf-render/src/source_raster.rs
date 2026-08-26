//! Native source-grid polygon scan conversion.
//!
//! This consumes typed projected vertices and materials at fixed-update
//! presentation boundaries. It is a renderer, not source-machine state.

use crate::gpu::{Gpu, TextureId, Vertex2};
use crate::shapes::PalettePairStyle;
use crate::source_projection::ProjectedPoint;
use sf_core::point_field::PointPixel;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
const CHANNELS: usize = 4;

/// A source-bitmap block owned by a later HUD copy. Clearing it before the
/// 3D layer is submitted preserves the source's replacement semantics while
/// the HD renderer can still alpha-blend the decoded HUD art afterward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceBitmapRect {
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub height: usize,
}
const PLAYFIELD_LEFT: i16 = 16;
const PLAYFIELD_TOP: i16 = 16;
const PLAYFIELD_WIDTH: i16 = 224;
const PLAYFIELD_HEIGHT: i16 = 192;
const PLAYFIELD_RIGHT: i16 = PLAYFIELD_LEFT + PLAYFIELD_WIDTH - 1;
const PLAYFIELD_BOTTOM: i16 = PLAYFIELD_TOP + PLAYFIELD_HEIGHT - 1;
const EDGE_FRACTION_BITS: u32 = 8;
const EDGE_RECIPROCAL_ONE: i32 = 32_768;
const EDGE_UNIT_RECIPROCAL: i32 = 32_767;
const EDGE_ROUNDING_BIAS: i32 = 127;
const EDGE_LEFT_CLAMP_DISTANCE: i32 = 8 << EDGE_FRACTION_BITS;
const TEXTURE_ROW_STRIDE: usize = 256;
const TEXTURE_BANK_MASK: usize = 32_767;
const SOURCE_PALETTE_MAX_INDEX: u8 = 15;
const SOURCE_CLEAR_INDEX: u8 = 0;
pub const NO_FACE: u16 = u16::MAX;
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn average_palette_pair(palette: &[[f32; 4]; 16], pair: [u8; 2]) -> [f32; 4] {
    let low = palette[usize::from(pair[0].min(SOURCE_PALETTE_MAX_INDEX))];
    let high = palette[usize::from(pair[1].min(SOURCE_PALETTE_MAX_INDEX))];
    [
        (low[0] + high[0]) * 0.5,
        (low[1] + high[1]) * 0.5,
        (low[2] + high[2]) * 0.5,
        1.0,
    ]
}

#[derive(Debug)]
pub struct SourceRaster {
    rgba: Vec<u8>,
    indices: Vec<u8>,
    owners: Vec<u16>,
    faces: Vec<u16>,
    current_owner: u16,
    current_face: u16,
    has_pixels: bool,
    palette_pair_style: PalettePairStyle,
}

#[derive(Debug, Clone, Copy)]
enum EdgeDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, Copy)]
enum ClipBoundary {
    Left,
    Right,
    Top,
    Bottom,
}

impl ClipBoundary {
    fn contains(self, point: ProjectedPoint) -> bool {
        match self {
            Self::Left => point.x >= PLAYFIELD_LEFT,
            // Source outcodes classify x == right and y == bottom as outside;
            // crossing edges still intersect on those final drawable pixels.
            Self::Right => point.x < PLAYFIELD_RIGHT,
            Self::Top => point.y >= PLAYFIELD_TOP,
            Self::Bottom => point.y < PLAYFIELD_BOTTOM,
        }
    }

    /// The source's dedicated two-point clipper tests its maximum clip
    /// coordinates after subtracting one, so the final column and row are
    /// inclusive for lines even though polygon outcodes remain exclusive.
    fn contains_line_endpoint(self, point: ProjectedPoint) -> bool {
        match self {
            Self::Right => point.x <= PLAYFIELD_RIGHT,
            Self::Bottom => point.y <= PLAYFIELD_BOTTOM,
            Self::Left | Self::Top => self.contains(point),
        }
    }

    fn intersection(self, inside: ProjectedPoint, outside: ProjectedPoint) -> ProjectedPoint {
        let interpolate = |inside_axis: i16,
                           outside_axis: i16,
                           inside_value: i16,
                           outside_value: i16,
                           boundary: i16| {
            let axis_delta = i32::from(outside_axis) - i32::from(inside_axis);
            let distance = i32::from(boundary) - i32::from(inside_axis);
            let value_delta = i32::from(outside_value) - i32::from(inside_value);
            (i32::from(inside_value) + distance * value_delta / axis_delta) as i16
        };
        match self {
            Self::Left => ProjectedPoint {
                x: PLAYFIELD_LEFT,
                y: interpolate(inside.x, outside.x, inside.y, outside.y, PLAYFIELD_LEFT),
                depth: inside.depth,
            },
            Self::Right => ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: interpolate(inside.x, outside.x, inside.y, outside.y, PLAYFIELD_RIGHT),
                depth: inside.depth,
            },
            Self::Top => ProjectedPoint {
                x: interpolate(inside.y, outside.y, inside.x, outside.x, PLAYFIELD_TOP),
                y: PLAYFIELD_TOP,
                depth: inside.depth,
            },
            Self::Bottom => ProjectedPoint {
                x: interpolate(inside.y, outside.y, inside.x, outside.x, PLAYFIELD_BOTTOM),
                y: PLAYFIELD_BOTTOM,
                depth: inside.depth,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EdgeWalker {
    vertex_index: usize,
    fixed_x: i32,
    step: i32,
    rows_remaining: i32,
    first_segment: bool,
    direction: EdgeDirection,
}

#[derive(Debug, Clone, Copy)]
struct TexturedVertex {
    point: ProjectedPoint,
    texture: [i16; 2],
}

#[derive(Debug, Clone, Copy)]
struct TexturedEdgeWalker {
    edge: EdgeWalker,
    fixed_texture: [i32; 2],
    texture_step: [i32; 2],
}

impl EdgeWalker {
    fn new(vertex_index: usize, x: i16, direction: EdgeDirection) -> Self {
        Self {
            vertex_index,
            fixed_x: i32::from(x) << EDGE_FRACTION_BITS,
            step: 0,
            rows_remaining: 0,
            first_segment: true,
            direction,
        }
    }

    fn prepare_segment(&mut self, vertices: &[ProjectedPoint], source_y: i16) -> bool {
        let mut start_x = if self.first_segment {
            i32::from(vertices[self.vertex_index].x)
        } else {
            (self.fixed_x + EDGE_ROUNDING_BIAS) >> EDGE_FRACTION_BITS
        };
        self.first_segment = false;

        loop {
            self.vertex_index = match self.direction {
                EdgeDirection::Forward => (self.vertex_index + 1) % vertices.len(),
                EdgeDirection::Reverse => (self.vertex_index + vertices.len() - 1) % vertices.len(),
            };
            let target = vertices[self.vertex_index];
            let delta_y = i32::from(target.y) - i32::from(source_y);
            if delta_y < 0 {
                return false;
            }
            if delta_y == 0 {
                start_x = i32::from(target.x);
                continue;
            }

            let reciprocal = if delta_y == 1 {
                EDGE_UNIT_RECIPROCAL
            } else {
                EDGE_RECIPROCAL_ONE / delta_y
            };
            let delta_x = i32::from(target.x) - start_x;
            self.step = (delta_x * reciprocal * 2) >> EDGE_FRACTION_BITS;
            self.fixed_x = start_x << EDGE_FRACTION_BITS;
            self.rows_remaining = delta_y;
            return true;
        }
    }

    fn advance(&mut self) {
        self.fixed_x += self.step;
        let playfield_left = i32::from(PLAYFIELD_LEFT) << EDGE_FRACTION_BITS;
        if self.fixed_x < playfield_left
            && self.fixed_x + EDGE_LEFT_CLAMP_DISTANCE >= playfield_left
        {
            self.fixed_x = playfield_left;
        }
        self.rows_remaining -= 1;
    }
}

impl TexturedEdgeWalker {
    fn new(vertex_index: usize, vertex: TexturedVertex, direction: EdgeDirection) -> Self {
        Self {
            edge: EdgeWalker::new(vertex_index, vertex.point.x, direction),
            fixed_texture: vertex
                .texture
                .map(|coordinate| i32::from(coordinate) << EDGE_FRACTION_BITS),
            texture_step: [0; 2],
        }
    }

    fn prepare_segment(&mut self, vertices: &[TexturedVertex], source_y: i16) -> bool {
        let mut start_x = if self.edge.first_segment {
            i32::from(vertices[self.edge.vertex_index].point.x)
        } else {
            (self.edge.fixed_x + EDGE_ROUNDING_BIAS) >> EDGE_FRACTION_BITS
        };
        let mut start_texture = if self.edge.first_segment {
            vertices[self.edge.vertex_index].texture.map(i32::from)
        } else {
            self.fixed_texture
                .map(|coordinate| coordinate >> EDGE_FRACTION_BITS)
        };
        self.edge.first_segment = false;

        loop {
            self.edge.vertex_index = match self.edge.direction {
                EdgeDirection::Forward => (self.edge.vertex_index + 1) % vertices.len(),
                EdgeDirection::Reverse => {
                    (self.edge.vertex_index + vertices.len() - 1) % vertices.len()
                }
            };
            let target = vertices[self.edge.vertex_index];
            let delta_y = i32::from(target.point.y) - i32::from(source_y);
            if delta_y < 0 {
                return false;
            }
            if delta_y == 0 {
                start_x = i32::from(target.point.x);
                start_texture = target.texture.map(i32::from);
                continue;
            }

            let reciprocal = if delta_y == 1 {
                EDGE_UNIT_RECIPROCAL
            } else {
                EDGE_RECIPROCAL_ONE / delta_y
            };
            self.edge.step =
                ((i32::from(target.point.x) - start_x) * reciprocal * 2) >> EDGE_FRACTION_BITS;
            self.edge.fixed_x = start_x << EDGE_FRACTION_BITS;
            for axis in 0..2 {
                self.texture_step[axis] =
                    ((i32::from(target.texture[axis]) - start_texture[axis]) * reciprocal * 2)
                        >> EDGE_FRACTION_BITS;
                self.fixed_texture[axis] = start_texture[axis] << EDGE_FRACTION_BITS;
            }
            self.edge.rows_remaining = delta_y;
            return true;
        }
    }

    fn advance(&mut self) {
        self.edge.advance();
        for axis in 0..2 {
            self.fixed_texture[axis] += self.texture_step[axis];
        }
    }
}

impl SourceRaster {
    pub fn new() -> Self {
        Self {
            rgba: vec![0; WIDTH * HEIGHT * CHANNELS],
            indices: vec![0; WIDTH * HEIGHT],
            owners: vec![0; WIDTH * HEIGHT],
            faces: vec![NO_FACE; WIDTH * HEIGHT],
            current_owner: 0,
            current_face: NO_FACE,
            has_pixels: false,
            palette_pair_style: PalettePairStyle::RetailDithered,
        }
    }

    pub fn with_palette_pair_style(palette_pair_style: PalettePairStyle) -> Self {
        Self {
            palette_pair_style,
            ..Self::new()
        }
    }

    pub fn indices(&self) -> &[u8] {
        &self.indices
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn owners(&self) -> &[u16] {
        &self.owners
    }

    pub fn faces(&self) -> &[u16] {
        &self.faces
    }

    pub fn set_owner(&mut self, owner: u16) {
        self.current_owner = owner;
    }

    pub fn set_face(&mut self, face: u16) {
        self.current_face = face;
    }

    /// Seed the cartridge bitmap with the source-projected point field. The
    /// retail renderer draws these pixels before shadows and normal objects,
    /// so later source-raster writes intentionally replace them.
    pub fn draw_point_field(&mut self, pixels: &[PointPixel], palette: &[[f32; 3]; 16]) {
        for point in pixels {
            let x = usize::from(point.x) + PLAYFIELD_LEFT as usize;
            let y = usize::from(point.y) + PLAYFIELD_TOP as usize;
            if x >= WIDTH || y >= HEIGHT {
                continue;
            }
            let palette_index = usize::from(point.palette_index.min(15));
            let color = palette[palette_index];
            let pixel = y * WIDTH + x;
            let rgba = pixel * CHANNELS;
            self.rgba[rgba..rgba + CHANNELS]
                .copy_from_slice(&rgba8([color[0], color[1], color[2], 1.0]));
            self.indices[pixel] = point.palette_index;
            self.owners[pixel] = 0;
            self.faces[pixel] = NO_FACE;
            self.has_pixels = true;
        }
    }

    pub(crate) fn clear_rect(&mut self, rect: SourceBitmapRect) {
        let right = rect.left.saturating_add(rect.width).min(WIDTH);
        let bottom = rect.top.saturating_add(rect.height).min(HEIGHT);
        for y in rect.top.min(HEIGHT)..bottom {
            for x in rect.left.min(WIDTH)..right {
                let pixel = y * WIDTH + x;
                let rgba = pixel * CHANNELS;
                self.rgba[rgba..rgba + CHANNELS].fill(0);
                self.indices[pixel] = 0;
                self.owners[pixel] = 0;
                self.faces[pixel] = NO_FACE;
            }
        }
    }

    #[cfg(test)]
    fn diagnostic_pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let offset = (y * WIDTH + x) * CHANNELS;
        self.rgba[offset..offset + CHANNELS]
            .try_into()
            .expect("diagnostic pixel has four channels")
    }

    pub fn draw_palette_pair(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        if self.palette_pair_style == PalettePairStyle::Smooth {
            self.draw_solid(points, indices, average_palette_pair(palette, pair));
            return;
        }
        self.draw_polygon(points, indices, |x, y| {
            let index = if (x ^ y) & 1 == 0 { pair[0] } else { pair[1] };
            Some(source_flat_palette_pixel(palette, index))
        });
    }

    pub fn draw_palette_line(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        if self.palette_pair_style == PalettePairStyle::Smooth {
            self.draw_solid_line(points, indices, average_palette_pair(palette, pair));
            return;
        }
        self.draw_line(points, indices, |x, y| {
            let index = if (x ^ y) & 1 == 0 { pair[0] } else { pair[1] };
            Some(source_flat_palette_pixel(palette, index))
        });
    }

    pub fn draw_solid(&mut self, points: &[ProjectedPoint], indices: &[u16], color: [f32; 4]) {
        let color = rgba8(color);
        self.draw_polygon(points, indices, |_, _| {
            (color[3] != 0).then_some((color, u8::MAX))
        });
    }

    pub fn draw_solid_line(&mut self, points: &[ProjectedPoint], indices: &[u16], color: [f32; 4]) {
        let color = rgba8(color);
        self.draw_line(points, indices, |_, _| {
            (color[3] != 0).then_some((color, u8::MAX))
        });
    }

    fn draw_line(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        mut color_at: impl FnMut(usize, usize) -> Option<([u8; 4], u8)>,
    ) {
        let [first_index, second_index] = indices else {
            return;
        };
        let (Some(first), Some(second)) = (
            points.get(usize::from(*first_index)),
            points.get(usize::from(*second_index)),
        ) else {
            return;
        };
        let Some([first, second]) = clip_line(*first, *second) else {
            return;
        };
        let (mut x, mut y) = (i32::from(first.x), i32::from(first.y));
        let (end_x, end_y) = (i32::from(second.x), i32::from(second.y));
        let delta_x = (end_x - x).abs();
        let delta_y = (end_y - y).abs();
        let step_x = (end_x - x).signum();
        let step_y = (end_y - y).signum();

        let mut plot = |x: i32, y: i32| {
            let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
                return;
            };
            if x >= WIDTH || y >= HEIGHT {
                return;
            }
            let Some((color, index)) = color_at(x, y) else {
                return;
            };
            let pixel = y * WIDTH + x;
            let offset = pixel * CHANNELS;
            self.rgba[offset..offset + CHANNELS].copy_from_slice(&color);
            self.indices[pixel] = index;
            self.owners[pixel] = self.current_owner;
            self.faces[pixel] = self.current_face;
            self.has_pixels |= color[3] != 0;
        };

        if delta_x >= delta_y {
            // The source line primitive tests the signed accumulator before
            // advancing the minor axis, then subtracts the minor distance at
            // the end of each major-axis step.
            let mut error = (delta_x - delta_y) / 2 - delta_y;
            for _ in 0..=delta_x {
                plot(x, y);
                if error < 0 {
                    y += step_y;
                    error += delta_x;
                }
                x += step_x;
                error -= delta_y;
            }
        } else {
            let mut error = (delta_y - delta_x) / 2 - delta_x;
            for _ in 0..=delta_y {
                plot(x, y);
                if error < 0 {
                    x += step_x;
                    error += delta_y;
                }
                y += step_y;
                error -= delta_x;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_scaled_sprite(
        &mut self,
        top_left: [i16; 2],
        projected_size: u16,
        source_size: u16,
        texture: &[u8],
        texture_offset: u16,
        high_nibble: bool,
        palette: &[[f32; 4]; 16],
    ) {
        if projected_size == 0 || source_size == 0 {
            return;
        }
        let projected_size_u32 = u32::from(projected_size);
        let source_size_u32 = u32::from(source_size);
        let reduction_step = (source_size_u32 << 8) / projected_size_u32;
        for destination_y in 0..projected_size {
            // The enlargement routine advances its vertical texture row
            // before selecting repeated output rows; the reduction routine
            // uses the ordinary floor accumulator.
            let source_y = if projected_size > source_size {
                (u32::from(destination_y) * source_size_u32 + projected_size_u32.saturating_sub(1))
                    / projected_size_u32
            } else {
                (u32::from(destination_y) * reduction_step) >> 8
            };
            let source_y = source_y.min(source_size_u32 - 1);
            let screen_y = top_left[1].wrapping_add(destination_y as i16);
            if !(PLAYFIELD_TOP..=PLAYFIELD_BOTTOM).contains(&screen_y) {
                continue;
            }
            for destination_x in 0..projected_size {
                let source_x = if projected_size > source_size {
                    u32::from(destination_x) * source_size_u32 / projected_size_u32
                } else {
                    (u32::from(destination_x) * reduction_step) >> 8
                };
                let screen_x = top_left[0].wrapping_add(destination_x as i16);
                if !(PLAYFIELD_LEFT..=PLAYFIELD_RIGHT).contains(&screen_x) {
                    continue;
                }
                let address = (usize::from(texture_offset)
                    + source_y as usize * TEXTURE_ROW_STRIDE
                    + source_x as usize)
                    & TEXTURE_BANK_MASK;
                let Some(&texel) = texture.get(address) else {
                    continue;
                };
                let palette_index = if high_nibble { texel >> 4 } else { texel & 15 };
                if palette_index == 0 {
                    continue;
                }
                let x = screen_x as usize;
                let y = screen_y as usize;
                let offset = (y * WIDTH + x) * CHANNELS;
                self.rgba[offset..offset + CHANNELS]
                    .copy_from_slice(&rgba8(palette[usize::from(palette_index)]));
                self.indices[y * WIDTH + x] = palette_index;
                self.owners[y * WIDTH + x] = self.current_owner;
                self.faces[y * WIDTH + x] = self.current_face;
                self.has_pixels = true;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_textured_polygon(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        texture_coordinates: &[[u8; 2]],
        texture: &[u8],
        texture_offset: u16,
        texture_mask: u16,
        high_nibble: bool,
        texture_scroll: [u8; 2],
        palette: &[[f32; 4]; 16],
    ) {
        if indices.len() < 3 || indices.len() != texture_coordinates.len() {
            return;
        }
        let vertices = indices
            .iter()
            .zip(texture_coordinates)
            .map(|(index, texture)| {
                points
                    .get(usize::from(*index))
                    .copied()
                    .map(|point| TexturedVertex {
                        point,
                        texture: texture.map(i16::from),
                    })
            })
            .collect::<Option<Vec<_>>>();
        let Some(vertices) = vertices else {
            return;
        };
        let vertices = clip_textured_polygon(vertices);
        if vertices.len() < 3 {
            return;
        }
        let minimum_vertex = vertices
            .iter()
            .enumerate()
            .min_by_key(|(_, vertex)| vertex.point.y)
            .map(|(index, _)| index)
            .unwrap_or(0);
        let minimum_y = vertices[minimum_vertex].point.y;
        let maximum_y = vertices
            .iter()
            .map(|vertex| vertex.point.y)
            .max()
            .unwrap_or(0);
        if minimum_y == maximum_y {
            return;
        }

        let mut forward = TexturedEdgeWalker::new(
            minimum_vertex,
            vertices[minimum_vertex],
            EdgeDirection::Forward,
        );
        let mut reverse = TexturedEdgeWalker::new(
            minimum_vertex,
            vertices[minimum_vertex],
            EdgeDirection::Reverse,
        );
        for source_y in minimum_y..maximum_y {
            if forward.edge.rows_remaining == 0 && !forward.prepare_segment(&vertices, source_y) {
                break;
            }
            if reverse.edge.rows_remaining == 0 && !reverse.prepare_segment(&vertices, source_y) {
                break;
            }

            let left = forward.edge.fixed_x >> EDGE_FRACTION_BITS;
            let right = reverse.edge.fixed_x >> EDGE_FRACTION_BITS;
            let span = right - left + 1;
            if span > 0 {
                let reciprocal = if span == 1 {
                    EDGE_UNIT_RECIPROCAL
                } else {
                    EDGE_RECIPROCAL_ONE / span
                };
                let mut fixed_texture = forward.fixed_texture;
                let texture_step: [i32; 2] = std::array::from_fn(|axis| {
                    let difference = reverse.fixed_texture[axis] - fixed_texture[axis];
                    ((i64::from(difference) * i64::from(reciprocal) * 2) >> 16) as i32
                });
                for source_x in left..=right {
                    let texture_coordinate: [usize; 2] = std::array::from_fn(|axis| {
                        let scrolled = (fixed_texture[axis] as i16)
                            .wrapping_add(i16::from(texture_scroll[axis]) << EDGE_FRACTION_BITS);
                        usize::from((scrolled as u16 >> EDGE_FRACTION_BITS) as u8)
                    });
                    let texture_x = texture_coordinate[0] & usize::from(texture_mask & 255);
                    let texture_y =
                        texture_coordinate[1] & usize::from(texture_mask >> EDGE_FRACTION_BITS);
                    let address =
                        (usize::from(texture_offset) + texture_y * TEXTURE_ROW_STRIDE + texture_x)
                            & TEXTURE_BANK_MASK;
                    let texel = texture.get(address).copied().unwrap_or(0);
                    let palette_index = if high_nibble { texel >> 4 } else { texel & 15 };
                    if palette_index != 0 {
                        if let (Ok(x), Ok(y)) =
                            (usize::try_from(source_x), usize::try_from(source_y))
                        {
                            if x < WIDTH && y < HEIGHT {
                                let pixel = y * WIDTH + x;
                                let offset = pixel * CHANNELS;
                                self.rgba[offset..offset + CHANNELS]
                                    .copy_from_slice(&rgba8(palette[usize::from(palette_index)]));
                                self.indices[pixel] = palette_index;
                                self.owners[pixel] = self.current_owner;
                                self.faces[pixel] = self.current_face;
                                self.has_pixels = true;
                            }
                        }
                    }
                    for axis in 0..2 {
                        fixed_texture[axis] += texture_step[axis];
                    }
                }
            }
            forward.advance();
            reverse.advance();
        }
    }

    fn draw_polygon(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        mut color_at: impl FnMut(usize, usize) -> Option<([u8; 4], u8)>,
    ) {
        if indices.len() < 3 {
            return;
        }
        let vertices: Vec<_> = indices
            .iter()
            .filter_map(|index| points.get(usize::from(*index)).copied())
            .collect();
        if vertices.len() != indices.len() {
            return;
        }
        let vertices = clip_polygon(vertices);
        if vertices.len() < 3 {
            return;
        }
        let minimum_vertex = vertices
            .iter()
            .enumerate()
            .min_by_key(|(_, point)| point.y)
            .map(|(index, _)| index)
            .unwrap_or(0);
        let minimum_y = vertices[minimum_vertex].y;
        let maximum_y = vertices.iter().map(|point| point.y).max().unwrap_or(0);
        if minimum_y == maximum_y {
            return;
        }

        let mut forward = EdgeWalker::new(
            minimum_vertex,
            vertices[minimum_vertex].x,
            EdgeDirection::Forward,
        );
        let mut reverse = EdgeWalker::new(
            minimum_vertex,
            vertices[minimum_vertex].x,
            EdgeDirection::Reverse,
        );
        for source_y in minimum_y..maximum_y {
            if forward.rows_remaining == 0 && !forward.prepare_segment(&vertices, source_y) {
                break;
            }
            if reverse.rows_remaining == 0 && !reverse.prepare_segment(&vertices, source_y) {
                break;
            }

            let Ok(y) = usize::try_from(source_y) else {
                forward.advance();
                reverse.advance();
                continue;
            };
            if y < HEIGHT {
                let left = forward.fixed_x >> EDGE_FRACTION_BITS;
                let right = reverse.fixed_x >> EDGE_FRACTION_BITS;
                for source_x in left..=right {
                    let Ok(x) = usize::try_from(source_x) else {
                        continue;
                    };
                    if x >= WIDTH {
                        continue;
                    }
                    let Some((color, index)) = color_at(x, y) else {
                        continue;
                    };
                    let offset = (y * WIDTH + x) * CHANNELS;
                    self.rgba[offset..offset + CHANNELS].copy_from_slice(&color);
                    self.indices[y * WIDTH + x] = index;
                    self.owners[y * WIDTH + x] = self.current_owner;
                    self.faces[y * WIDTH + x] = self.current_face;
                    self.has_pixels |= color[3] != 0;
                }
            }
            forward.advance();
            reverse.advance();
        }
    }

    pub fn submit(
        &self,
        gpu: &mut Gpu,
        texture: &mut Option<TextureId>,
        output_width: u32,
        output_height: u32,
        presentation_offset: [i16; 2],
    ) {
        if !self.has_pixels || output_width == 0 || output_height == 0 {
            return;
        }
        let texture = if let Some(texture) = *texture {
            gpu.update_texture(texture, &self.rgba);
            texture
        } else {
            let created = gpu.create_texture_rgba(WIDTH as u32, HEIGHT as u32, &self.rgba);
            *texture = Some(created);
            created
        };
        let scale = output_height as f32 / HEIGHT as f32;
        let draw_width = WIDTH as f32 * scale;
        let left =
            (output_width as f32 - draw_width) * 0.5 + f32::from(presentation_offset[0]) * scale;
        let top = f32::from(presentation_offset[1]) * scale;
        let vertices = [
            Vertex2 {
                pos: [left, top],
                uv: [0.0, 1.0],
            },
            Vertex2 {
                pos: [left + draw_width, top],
                uv: [1.0, 1.0],
            },
            Vertex2 {
                pos: [left + draw_width, output_height as f32 + top],
                uv: [1.0, 0.0],
            },
            Vertex2 {
                pos: [left, output_height as f32 + top],
                uv: [0.0, 0.0],
            },
        ];
        let projection = [
            2.0 / output_width as f32,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / output_height as f32,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            -1.0,
            -1.0,
            0.0,
            1.0,
        ];
        gpu.push_overlay_fan(
            &vertices,
            &projection,
            &IDENTITY,
            [1.0; 4],
            1,
            None,
            texture,
        );
    }
}

fn clip_polygon(mut input: Vec<ProjectedPoint>) -> Vec<ProjectedPoint> {
    for boundary in [
        ClipBoundary::Left,
        ClipBoundary::Right,
        ClipBoundary::Top,
        ClipBoundary::Bottom,
    ] {
        if input.is_empty() {
            break;
        }
        let mut output = Vec::with_capacity(input.len() + 2);
        let mut previous = *input.last().expect("nonempty polygon");
        let mut previous_inside = boundary.contains(previous);
        for current in input.iter().copied() {
            let current_inside = boundary.contains(current);
            match (previous_inside, current_inside) {
                (true, true) => output.push(current),
                (true, false) => output.push(boundary.intersection(previous, current)),
                (false, true) => {
                    let crossing = match boundary {
                        // The source's horizontal-entry path evaluates from
                        // the outside endpoint, so signed division truncation
                        // selects the opposite adjacent integer.
                        ClipBoundary::Top | ClipBoundary::Bottom => {
                            boundary.intersection(previous, current)
                        }
                        ClipBoundary::Left | ClipBoundary::Right => {
                            boundary.intersection(current, previous)
                        }
                    };
                    output.push(crossing);
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
            previous_inside = current_inside;
        }
        input = output;
    }
    input
}

fn textured_intersection(
    boundary: ClipBoundary,
    inside: TexturedVertex,
    outside: TexturedVertex,
) -> TexturedVertex {
    let point = boundary.intersection(inside.point, outside.point);
    let (inside_axis, outside_axis, boundary_axis) = match boundary {
        ClipBoundary::Left | ClipBoundary::Right => (inside.point.x, outside.point.x, point.x),
        ClipBoundary::Top | ClipBoundary::Bottom => (inside.point.y, outside.point.y, point.y),
    };
    let axis_delta = i32::from(outside_axis) - i32::from(inside_axis);
    let distance = i32::from(boundary_axis) - i32::from(inside_axis);
    let texture = std::array::from_fn(|axis| {
        let value_delta = i32::from(outside.texture[axis]) - i32::from(inside.texture[axis]);
        (i32::from(inside.texture[axis]) + distance * value_delta / axis_delta) as i16
    });
    TexturedVertex { point, texture }
}

fn clip_textured_polygon(mut input: Vec<TexturedVertex>) -> Vec<TexturedVertex> {
    for boundary in [
        ClipBoundary::Left,
        ClipBoundary::Right,
        ClipBoundary::Top,
        ClipBoundary::Bottom,
    ] {
        if input.is_empty() {
            break;
        }
        let mut output = Vec::with_capacity(input.len() + 2);
        let mut previous = *input.last().expect("nonempty textured polygon");
        let mut previous_inside = boundary.contains(previous.point);
        for current in input.iter().copied() {
            let current_inside = boundary.contains(current.point);
            match (previous_inside, current_inside) {
                (true, true) => output.push(current),
                (true, false) => {
                    output.push(textured_intersection(boundary, previous, current));
                }
                (false, true) => {
                    // Texture clipping evaluates from the authored previous
                    // endpoint. Keeping that direction matters because signed
                    // division truncates adjacent intersections differently.
                    let crossing = textured_intersection(boundary, previous, current);
                    output.push(crossing);
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
            previous_inside = current_inside;
        }
        input = output;
    }
    input
}

fn clip_line(first: ProjectedPoint, second: ProjectedPoint) -> Option<[ProjectedPoint; 2]> {
    // MCLIP's dedicated two-point path feeds a closed two-vertex stream
    // through the polygon clipper.  That is observably different from a
    // conventional line clip when an edge passes just outside a corner: the
    // two authored directions can truncate onto the corner and leave a
    // zero-length line there.  Preserve that presentation result without
    // carrying any of the source processor's working state into the port.
    let mut input = vec![first, second];
    for boundary in [
        ClipBoundary::Left,
        ClipBoundary::Right,
        ClipBoundary::Top,
        ClipBoundary::Bottom,
    ] {
        let mut output = Vec::with_capacity(input.len() + 2);
        for index in 0..input.len() {
            let edge_start = input[index];
            let edge_end = input[(index + 1) % input.len()];
            match (
                boundary.contains_line_endpoint(edge_start),
                boundary.contains_line_endpoint(edge_end),
            ) {
                (true, true) => output.push(edge_start),
                (true, false) => {
                    output.push(edge_start);
                    output.push(boundary.intersection(edge_start, edge_end));
                }
                (false, true) => {
                    output.push(boundary.intersection(edge_start, edge_end));
                }
                (false, false) => {}
            }
        }
        if output.len() < 2 {
            return None;
        }
        input = output;
    }
    Some([input[0], input[1]])
}

fn rgba8(color: [f32; 4]) -> [u8; 4] {
    color.map(|component| (component.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn source_flat_palette_pixel(palette: &[[f32; 4]; 16], index: u8) -> ([u8; 4], u8) {
    if index == SOURCE_CLEAR_INDEX {
        // Flat primitives replace destination bitplanes even when their
        // authored color is zero. Textures and sprites handle zero separately
        // as a transparent sample before reaching the pixel writer.
        return ([0; CHANNELS], SOURCE_CLEAR_INDEX);
    }
    (
        rgba8(palette[usize::from(index.min(SOURCE_PALETTE_MAX_INDEX))]),
        index,
    )
}

impl Default for SourceRaster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_palette_pairs_replace_checkerboards_with_the_average_color() {
        const POINTS: [ProjectedPoint; 4] = [
            ProjectedPoint {
                x: 80,
                y: 80,
                depth: 1,
            },
            ProjectedPoint {
                x: 96,
                y: 80,
                depth: 1,
            },
            ProjectedPoint {
                x: 96,
                y: 96,
                depth: 1,
            },
            ProjectedPoint {
                x: 80,
                y: 96,
                depth: 1,
            },
        ];
        const FACE: [u16; 4] = [0, 3, 2, 1];
        const LOW_COLOR: u8 = 1;
        const HIGH_COLOR: u8 = 2;
        const TARGET_X: usize = 88;
        const TARGET_Y: usize = 88;

        let mut palette = [[0.0; 4]; 16];
        palette[usize::from(LOW_COLOR)] = [0.2, 0.4, 0.6, 1.0];
        palette[usize::from(HIGH_COLOR)] = [0.6, 0.8, 1.0, 1.0];
        let mut raster = SourceRaster::with_palette_pair_style(PalettePairStyle::Smooth);
        raster.draw_palette_pair(&POINTS, &FACE, &palette, [LOW_COLOR, HIGH_COLOR]);

        assert_eq!(
            raster.diagnostic_pixel(TARGET_X, TARGET_Y),
            rgba8([0.4, 0.6, 0.8, 1.0])
        );
        assert_eq!(
            raster.diagnostic_pixel(TARGET_X + 1, TARGET_Y),
            rgba8([0.4, 0.6, 0.8, 1.0])
        );
    }

    #[test]
    fn opaque_source_pixel_replaces_the_background_exactly() {
        const TARGET_X: usize = 16;
        const TARGET_Y: usize = 105;
        const COLOR: [u8; 4] = [115, 132, 156, 255];

        let mut raster = SourceRaster::new();
        let offset = (TARGET_Y * WIDTH + TARGET_X) * CHANNELS;
        raster.rgba[offset..offset + CHANNELS].copy_from_slice(&COLOR);
        raster.has_pixels = true;

        let mut gpu = Gpu::new_headless(WIDTH as u32, HEIGHT as u32)
            .expect("headless source-raster renderer");
        gpu.set_clear_color(99.0 / 255.0, 181.0 / 255.0, 156.0 / 255.0, 1.0);
        gpu.begin_frame();
        let mut texture = None;
        raster.submit(&mut gpu, &mut texture, WIDTH as u32, HEIGHT as u32, [0, 0]);
        gpu.end_frame();

        let (_, _, pixels) = gpu.read_pixels().expect("source-raster pixels");
        let display_y = TARGET_Y;
        let display_offset = (display_y * WIDTH + TARGET_X) * CHANNELS;
        let matches = pixels
            .chunks_exact(CHANNELS)
            .enumerate()
            .filter_map(|(index, pixel)| (pixel == COLOR).then_some((index % WIDTH, index / WIDTH)))
            .collect::<Vec<_>>();
        assert_eq!(
            &pixels[display_offset..display_offset + CHANNELS],
            &COLOR,
            "opaque pixel locations: {matches:?}",
        );
    }

    #[test]
    fn training_pillar_texture_uses_the_authored_scanline_phase() {
        const POINTS: [ProjectedPoint; 4] = [
            ProjectedPoint {
                x: 69,
                y: 90,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 62,
                y: 90,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 62,
                y: 96,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 69,
                y: 96,
                depth: 5_119,
            },
        ];
        const INDICES: [u16; 4] = [0, 1, 2, 3];
        const TEXTURE_COORDINATES: [[u8; 2]; 4] = [[0, 0], [31, 0], [31, 31], [0, 31]];
        const EXPECTED: [[u8; 8]; 6] = [
            [10, 10, 10, 10, 10, 10, 10, 10],
            [10, 10, 10, 10, 14, 10, 10, 10],
            [10, 12, 12, 10, 14, 10, 12, 12],
            [10, 10, 10, 12, 10, 12, 10, 10],
            [10, 10, 10, 12, 10, 12, 10, 10],
            [10, 14, 10, 14, 10, 10, 10, 10],
        ];
        const TEXTURE_OFFSET: u16 = 192;
        const TEXTURE_MASK: u16 = 0x1F1F;
        const TEXTURE: &[u8; 32_768] =
            include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_01.BIN");
        let palette = [[0.0, 0.0, 0.0, 1.0]; 16];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(&POINTS, &INDICES, &palette, [10, 10]);
        raster.draw_textured_polygon(
            &POINTS,
            &INDICES,
            &TEXTURE_COORDINATES,
            TEXTURE,
            TEXTURE_OFFSET,
            TEXTURE_MASK,
            false,
            [0, 0],
            &palette,
        );

        for (row, expected) in EXPECTED.iter().enumerate() {
            let start = (90 + row) * WIDTH + 62;
            assert_eq!(&raster.indices[start..start + expected.len()], expected);
        }
    }

    #[test]
    fn clipped_training_pillar_texture_keeps_authored_intersection_direction() {
        const POINTS: [ProjectedPoint; 4] = [
            ProjectedPoint {
                x: 242,
                y: 73,
                depth: 2_977,
            },
            ProjectedPoint {
                x: 231,
                y: 73,
                depth: 2_977,
            },
            ProjectedPoint {
                x: 231,
                y: 84,
                depth: 2_977,
            },
            ProjectedPoint {
                x: 242,
                y: 84,
                depth: 2_977,
            },
        ];
        const INDICES: [u16; 4] = [0, 1, 2, 3];
        const TEXTURE_COORDINATES: [[u8; 2]; 4] = [[0, 0], [31, 0], [31, 31], [0, 31]];
        const TEXTURE: &[u8; 32_768] =
            include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_01.BIN");
        let palette = [[0.0, 0.0, 0.0, 1.0]; 16];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(&POINTS, &INDICES, &palette, [10, 10]);
        raster.draw_textured_polygon(
            &POINTS,
            &INDICES,
            &TEXTURE_COORDINATES,
            TEXTURE,
            192,
            0x1F1F,
            false,
            [0, 0],
            &palette,
        );

        assert_eq!(raster.indices[77 * WIDTH + 233], 12);
    }

    #[test]
    fn clipped_corneria_building_keeps_the_source_left_edge_guard() {
        const CAPTURED_POINTS: [ProjectedPoint; 12] = [
            ProjectedPoint {
                x: 16,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -3,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: 17,
                y: 75,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -2,
                y: 74,
                depth: 2_095,
            },
            ProjectedPoint {
                x: 9,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -11,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: 10,
                y: 72,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -11,
                y: 72,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -10,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: 6,
                y: 122,
                depth: 2_095,
            },
            ProjectedPoint {
                x: -9,
                y: 72,
                depth: 2_095,
            },
            ProjectedPoint {
                x: 6,
                y: 72,
                depth: 2_095,
            },
        ];
        const FACE: [u16; 4] = [2, 6, 4, 0];
        const SOURCE_COLOR: u8 = 11;

        let mut palette = [[0.0; 4]; 16];
        palette[usize::from(SOURCE_COLOR)] = [1.0; 4];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(
            &CAPTURED_POINTS,
            &FACE,
            &palette,
            [SOURCE_COLOR, SOURCE_COLOR],
        );

        for y in 118..=121 {
            assert_eq!(
                raster.indices()[y * WIDTH + PLAYFIELD_LEFT as usize],
                SOURCE_COLOR,
                "captured left-edge row {y}",
            );
        }
    }

    #[test]
    fn flat_palette_zero_clears_earlier_geometry() {
        const POINTS: [ProjectedPoint; 4] = [
            ProjectedPoint {
                x: 80,
                y: 80,
                depth: 1,
            },
            ProjectedPoint {
                x: 96,
                y: 80,
                depth: 1,
            },
            ProjectedPoint {
                x: 96,
                y: 96,
                depth: 1,
            },
            ProjectedPoint {
                x: 80,
                y: 96,
                depth: 1,
            },
        ];
        const FACE: [u16; 4] = [0, 3, 2, 1];
        const UNDER_COLOR: u8 = 7;
        const UNDER_OWNER: u16 = 3;
        const OVER_OWNER: u16 = 4;
        const TARGET_X: usize = 88;
        const TARGET_Y: usize = 88;

        let mut raster = SourceRaster::new();
        raster.set_owner(UNDER_OWNER);
        raster.draw_palette_pair(&POINTS, &FACE, &[[1.0; 4]; 16], [UNDER_COLOR, UNDER_COLOR]);
        raster.set_owner(OVER_OWNER);
        raster.draw_palette_pair(&POINTS, &FACE, &[[1.0; 4]; 16], [0, 0]);

        let target = TARGET_Y * WIDTH + TARGET_X;
        assert_eq!(raster.indices()[target], SOURCE_CLEAR_INDEX);
        assert_eq!(raster.owners()[target], OVER_OWNER);
        assert_eq!(raster.diagnostic_pixel(TARGET_X, TARGET_Y), [0; CHANNELS]);
    }

    #[test]
    fn scaled_sprite_sampling_matches_the_retail_frame_333_boost_pixel() {
        const TOP_LEFT: [i16; 2] = [70, 52];
        const PROJECTED_SIZE: u16 = 33;
        const SOURCE_SIZE: u16 = 32;
        const TEXTURE_OFFSET: u16 = 16_480;
        const TARGET: [usize; 2] = [83, 55];
        const SOURCE_SAMPLE: [usize; 2] = [12, 3];
        const PALETTE_INDEX: u8 = 6;

        let mut texture = [0u8; TEXTURE_BANK_MASK + 1];
        texture[usize::from(TEXTURE_OFFSET)
            + SOURCE_SAMPLE[1] * TEXTURE_ROW_STRIDE
            + SOURCE_SAMPLE[0]] = PALETTE_INDEX;
        let mut raster = SourceRaster::new();
        raster.set_owner(6);
        raster.draw_scaled_sprite(
            TOP_LEFT,
            PROJECTED_SIZE,
            SOURCE_SIZE,
            &texture,
            TEXTURE_OFFSET,
            false,
            &[[1.0; 4]; 16],
        );
        let offset = TARGET[1] * WIDTH + TARGET[0];
        assert_eq!(raster.indices()[offset], PALETTE_INDEX);
        assert_eq!(raster.owners()[offset], 6);
        assert_eq!(raster.faces()[offset], NO_FACE);
    }

    #[test]
    fn vertical_source_line_includes_both_retail_endpoints() {
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 78,
                y: 89,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 78,
                y: 113,
                depth: 5_119,
            },
        ];
        const COLOR_INDEX: u8 = 1;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        for y in 89..=113 {
            assert_eq!(raster.indices()[y * WIDTH + 78], COLOR_INDEX);
        }
        assert_eq!(raster.indices()[88 * WIDTH + 78], 0);
        assert_eq!(raster.indices()[114 * WIDTH + 78], 0);
    }

    #[test]
    fn diagonal_source_line_uses_the_retail_initial_step_phase() {
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 78,
                y: 91,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 82,
                y: 109,
                depth: 5_119,
            },
        ];
        const COLOR_INDEX: u8 = 2;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        assert_eq!(raster.indices()[93 * WIDTH + 79], COLOR_INDEX);
        assert_eq!(raster.indices()[93 * WIDTH + 78], 0);
    }

    #[test]
    fn negative_diagonal_source_line_defers_the_first_horizontal_step() {
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 78,
                y: 91,
                depth: 5_119,
            },
            ProjectedPoint {
                x: 73,
                y: 109,
                depth: 5_119,
            },
        ];
        const COLOR_INDEX: u8 = 9;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        assert_eq!(raster.indices()[92 * WIDTH + 78], COLOR_INDEX);
        assert_eq!(raster.indices()[92 * WIDTH + 77], 0);
        assert_eq!(raster.indices()[93 * WIDTH + 77], COLOR_INDEX);
    }

    #[test]
    fn clipped_training_tower_line_restarts_its_retail_step_phase() {
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 13,
                y: 114,
                depth: 2_788,
            },
            ProjectedPoint {
                x: 46,
                y: 107,
                depth: 2_788,
            },
        ];
        const COLOR_INDEX: u8 = 2;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        assert_eq!(raster.indices()[108 * WIDTH + 39], 0);
        assert_eq!(raster.indices()[109 * WIDTH + 39], COLOR_INDEX);
    }

    #[test]
    fn clipped_training_tower_line_preserves_authored_intersection_direction() {
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 25,
                y: 107,
                depth: 2_788,
            },
            ProjectedPoint {
                x: 13,
                y: 114,
                depth: 2_788,
            },
        ];
        const COLOR_INDEX: u8 = 3;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        assert_eq!(raster.indices()[108 * WIDTH + 23], COLOR_INDEX);
        assert_eq!(raster.indices()[109 * WIDTH + 23], 0);
    }

    #[test]
    fn source_line_corner_clip_retains_the_wireframe_origin_pixel() {
        // MY_W face 1 at Training frame 1089.  The geometric segment misses
        // the playfield, but the source's closed two-point clip stream
        // truncates both directional intersections onto its upper-left pixel.
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 17,
                y: 15,
                depth: 7_681,
            },
            ProjectedPoint {
                x: -3,
                y: 16,
                depth: 6_916,
            },
        ];
        const COLOR_INDEX: u8 = 7;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(
            &POINTS,
            &[0, 1],
            &[[1.0; 4]; 16],
            [COLOR_INDEX, COLOR_INDEX],
        );

        assert_eq!(
            raster.indices()[PLAYFIELD_TOP as usize * WIDTH + PLAYFIELD_LEFT as usize],
            COLOR_INDEX,
        );
        assert_eq!(
            raster
                .indices()
                .iter()
                .filter(|&&index| index == COLOR_INDEX)
                .count(),
            1,
        );
    }

    #[test]
    fn corneria_corridor_line_keeps_only_the_retail_bottom_edge_pixel() {
        // OP_0 face 23 at Corneria game frame 26. These source-projected
        // endpoints are byte-for-byte equal to the retail projection capture.
        // The two directional left-edge intersections differ by one row; the
        // inclusive line-only bottom clip collapses the retained pair onto
        // the final drawable row.
        const POINTS: [ProjectedPoint; 2] = [
            ProjectedPoint {
                x: 36,
                y: 223,
                depth: 1_581,
            },
            ProjectedPoint {
                x: 3,
                y: 195,
                depth: 1_581,
            },
        ];
        const EVEN_COLOR: u8 = 14;
        const ODD_COLOR: u8 = 13;

        let mut raster = SourceRaster::new();
        raster.draw_palette_line(&POINTS, &[0, 1], &[[1.0; 4]; 16], [EVEN_COLOR, ODD_COLOR]);

        assert_eq!(
            raster.indices()[PLAYFIELD_BOTTOM as usize * WIDTH + PLAYFIELD_LEFT as usize],
            ODD_COLOR,
        );
        assert_eq!(
            raster.indices()[(PLAYFIELD_BOTTOM as usize - 1) * WIDTH + PLAYFIELD_LEFT as usize],
            SOURCE_CLEAR_INDEX,
        );
        assert_eq!(
            raster
                .indices()
                .iter()
                .filter(|&&index| index != SOURCE_CLEAR_INDEX)
                .count(),
            1,
        );
    }

    #[test]
    fn bitmap_replacement_clears_color_and_diagnostics() {
        const TARGET: [usize; 2] = [70, 172];
        const OWNER: u16 = 9;
        const FACE: u16 = 4;
        const COLOR_INDEX: u8 = 13;
        const RECT: SourceBitmapRect = SourceBitmapRect {
            left: 64,
            top: 168,
            width: 48,
            height: 40,
        };

        let mut raster = SourceRaster::new();
        let pixel = TARGET[1] * WIDTH + TARGET[0];
        let rgba = pixel * CHANNELS;
        raster.rgba[rgba..rgba + CHANNELS].fill(255);
        raster.indices[pixel] = COLOR_INDEX;
        raster.owners[pixel] = OWNER;
        raster.faces[pixel] = FACE;

        raster.clear_rect(RECT);

        assert_eq!(&raster.rgba[rgba..rgba + CHANNELS], &[0; CHANNELS]);
        assert_eq!(raster.indices[pixel], 0);
        assert_eq!(raster.owners[pixel], 0);
        assert_eq!(raster.faces[pixel], NO_FACE);
    }

    #[test]
    fn reduced_high_nibble_sprite_matches_the_retail_frame_334_boost_pixel() {
        const TOP_LEFT: [i16; 2] = [76, 58];
        const PROJECTED_SIZE: u16 = 28;
        const SOURCE_SIZE: u16 = 32;
        const TEXTURE_OFFSET: u16 = 8_416;
        const TARGET: [usize; 2] = [91, 59];
        const SOURCE_SAMPLE: [usize; 2] = [17, 1];
        const PALETTE_INDEX: u8 = 5;

        let mut texture = [0u8; TEXTURE_BANK_MASK + 1];
        texture[usize::from(TEXTURE_OFFSET)
            + SOURCE_SAMPLE[1] * TEXTURE_ROW_STRIDE
            + SOURCE_SAMPLE[0]] = PALETTE_INDEX << 4;
        // Exact-ratio sampling would incorrectly select column 16 here; the
        // source's quantized reduction step selects transparent column 15.
        texture[usize::from(TEXTURE_OFFSET) + 4 * TEXTURE_ROW_STRIDE + 16] = PALETTE_INDEX << 4;
        let mut raster = SourceRaster::new();
        raster.draw_scaled_sprite(
            TOP_LEFT,
            PROJECTED_SIZE,
            SOURCE_SIZE,
            &texture,
            TEXTURE_OFFSET,
            true,
            &[[1.0; 4]; 16],
        );
        assert_eq!(
            raster.indices()[TARGET[1] * WIDTH + TARGET[0]],
            PALETTE_INDEX
        );
        assert_eq!(raster.indices()[62 * WIDTH + 90], 0);
    }

    #[test]
    fn clips_the_title_ship_face_with_source_integer_intersections() {
        let clipped = clip_polygon(vec![
            ProjectedPoint {
                x: 196,
                y: 175,
                depth: 1,
            },
            ProjectedPoint {
                x: 248,
                y: 213,
                depth: 1,
            },
            ProjectedPoint {
                x: 190,
                y: 160,
                depth: 1,
            },
        ]);
        let coordinates: Vec<_> = clipped.iter().map(|point| (point.x, point.y)).collect();
        assert_eq!(
            coordinates,
            [(196, 175), (239, 206), (239, 204), (190, 160)]
        );
    }

    #[test]
    fn clipped_title_ship_face_includes_the_retail_edge_pixel() {
        let points = [
            ProjectedPoint {
                x: 196,
                y: 177,
                depth: 1,
            },
            ProjectedPoint {
                x: 246,
                y: 215,
                depth: 1,
            },
            ProjectedPoint {
                x: 189,
                y: 162,
                depth: 1,
            },
        ];
        let clipped = clip_polygon(points.to_vec());
        let coordinates: Vec<_> = clipped.iter().map(|point| (point.x, point.y)).collect();
        assert_eq!(
            coordinates,
            [(196, 177), (236, 207), (238, 207), (189, 162)]
        );
        let mut raster = SourceRaster::new();
        raster.draw_solid(&points, &[0, 1, 2], [1.0; 4]);
        assert_eq!(raster.diagnostic_pixel(202, 174), [255; 4]);
    }

    #[test]
    fn wholly_right_of_source_bitmap_is_discarded() {
        let points = [
            ProjectedPoint {
                x: 258,
                y: 122,
                depth: 2_046,
            },
            ProjectedPoint {
                x: 259,
                y: 122,
                depth: 2_066,
            },
            ProjectedPoint {
                x: 259,
                y: 73,
                depth: 2_070,
            },
            ProjectedPoint {
                x: 257,
                y: 72,
                depth: 2_050,
            },
        ];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(&points, &[0, 1, 2, 3], &[[1.0; 4]; 16], [14, 14]);

        assert!(raster.indices().iter().all(|index| *index == 0));
    }

    #[test]
    fn face_touching_only_the_exclusive_right_boundary_is_discarded() {
        let points = [
            ProjectedPoint {
                x: 242,
                y: 72,
                depth: 2_050,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 73,
                depth: 2_070,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 122,
                depth: 2_062,
            },
            ProjectedPoint {
                x: 243,
                y: 122,
                depth: 2_042,
            },
        ];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(&points, &[0, 1, 2, 3], &[[1.0; 4]; 16], [12, 12]);

        assert!(raster.indices().iter().all(|index| *index == 0));
    }

    #[test]
    fn wholly_left_of_source_bitmap_is_discarded() {
        let points = [
            ProjectedPoint {
                x: -34,
                y: 7,
                depth: 2_000,
            },
            ProjectedPoint {
                x: -32,
                y: 7,
                depth: 2_020,
            },
            ProjectedPoint {
                x: -32,
                y: 28,
                depth: 2_020,
            },
            ProjectedPoint {
                x: -34,
                y: 28,
                depth: 2_000,
            },
        ];
        let mut raster = SourceRaster::new();
        raster.draw_palette_pair(&points, &[0, 1, 2, 3], &[[1.0; 4]; 16], [13, 13]);

        assert!(raster.indices().iter().all(|index| *index == 0));
    }
}
