//! Native source-grid polygon scan conversion.
//!
//! This consumes typed projected vertices and materials at fixed-update
//! presentation boundaries. It is a renderer, not source-machine state.

use crate::gpu::{Gpu, TextureId, Vertex2};
use crate::source_projection::ProjectedPoint;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
const CHANNELS: usize = 4;
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
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[derive(Debug)]
pub struct SourceRaster {
    rgba: Vec<u8>,
    has_pixels: bool,
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
            Self::Right => point.x <= PLAYFIELD_RIGHT,
            Self::Top => point.y >= PLAYFIELD_TOP,
            Self::Bottom => point.y <= PLAYFIELD_BOTTOM,
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
        self.rows_remaining -= 1;
    }
}

impl SourceRaster {
    pub fn new() -> Self {
        Self {
            rgba: vec![0; WIDTH * HEIGHT * CHANNELS],
            has_pixels: false,
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
        self.draw_polygon(points, indices, |x, y| {
            let index = if (x ^ y) & 1 == 0 { pair[0] } else { pair[1] };
            Some(if index == 0 {
                [0; CHANNELS]
            } else {
                rgba8(palette[usize::from(index.min(15))])
            })
        });
    }

    pub fn draw_solid(&mut self, points: &[ProjectedPoint], indices: &[u16], color: [f32; 4]) {
        let color = rgba8(color);
        self.draw_polygon(points, indices, |_, _| (color[3] != 0).then_some(color));
    }

    fn draw_polygon(
        &mut self,
        points: &[ProjectedPoint],
        indices: &[u16],
        mut color_at: impl FnMut(usize, usize) -> Option<[u8; 4]>,
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
                    let Some(color) = color_at(x, y) else {
                        continue;
                    };
                    let offset = (y * WIDTH + x) * CHANNELS;
                    self.rgba[offset..offset + CHANNELS].copy_from_slice(&color);
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
        let left = (output_width as f32 - draw_width) * 0.5;
        let vertices = [
            Vertex2 {
                pos: [left, 0.0],
                uv: [0.0, 1.0],
            },
            Vertex2 {
                pos: [left + draw_width, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex2 {
                pos: [left + draw_width, output_height as f32],
                uv: [1.0, 0.0],
            },
            Vertex2 {
                pos: [left, output_height as f32],
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

fn rgba8(color: [f32; 4]) -> [u8; 4] {
    color.map(|component| (component.clamp(0.0, 1.0) * 255.0).round() as u8)
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
}
