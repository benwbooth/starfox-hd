//! Native source-grid polygon scan conversion.
//!
//! This consumes typed projected vertices and materials at fixed-update
//! presentation boundaries. It is a renderer, not source-machine state.

use crate::gpu::{Gpu, TextureId, Vertex2};
use crate::source_projection::ProjectedPoint;

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
pub const NO_FACE: u16 = u16::MAX;
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[derive(Debug)]
pub struct SourceRaster {
    rgba: Vec<u8>,
    indices: Vec<u8>,
    owners: Vec<u16>,
    faces: Vec<u16>,
    current_owner: u16,
    current_face: u16,
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
            // Source outcodes classify x == right and y == bottom as outside;
            // crossing edges still intersect on those final drawable pixels.
            Self::Right => point.x < PLAYFIELD_RIGHT,
            Self::Top => point.y >= PLAYFIELD_TOP,
            Self::Bottom => point.y < PLAYFIELD_BOTTOM,
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
        let playfield_left = i32::from(PLAYFIELD_LEFT) << EDGE_FRACTION_BITS;
        if self.fixed_x < playfield_left
            && self.fixed_x + EDGE_LEFT_CLAMP_DISTANCE >= playfield_left
        {
            self.fixed_x = playfield_left;
        }
        self.rows_remaining -= 1;
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
        self.draw_polygon(points, indices, |x, y| {
            let index = if (x ^ y) & 1 == 0 { pair[0] } else { pair[1] };
            Some(if index == 0 {
                ([0; CHANNELS], index)
            } else {
                (rgba8(palette[usize::from(index.min(15))]), index)
            })
        });
    }

    pub fn draw_solid(&mut self, points: &[ProjectedPoint], indices: &[u16], color: [f32; 4]) {
        let color = rgba8(color);
        self.draw_polygon(points, indices, |_, _| {
            (color[3] != 0).then_some((color, u8::MAX))
        });
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
                (u32::from(destination_y) * source_size_u32
                    + projected_size_u32.saturating_sub(1))
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
        let left = (output_width as f32 - draw_width) * 0.5
            + f32::from(presentation_offset[0]) * scale;
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
        raster.submit(
            &mut gpu,
            &mut texture,
            WIDTH as u32,
            HEIGHT as u32,
            [0, 0],
        );
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
    fn clipped_corneria_building_keeps_the_source_left_edge_guard() {
        const CAPTURED_POINTS: [ProjectedPoint; 12] = [
            ProjectedPoint { x: 16, y: 122, depth: 2_095 },
            ProjectedPoint { x: -3, y: 122, depth: 2_095 },
            ProjectedPoint { x: 17, y: 75, depth: 2_095 },
            ProjectedPoint { x: -2, y: 74, depth: 2_095 },
            ProjectedPoint { x: 9, y: 122, depth: 2_095 },
            ProjectedPoint { x: -11, y: 122, depth: 2_095 },
            ProjectedPoint { x: 10, y: 72, depth: 2_095 },
            ProjectedPoint { x: -11, y: 72, depth: 2_095 },
            ProjectedPoint { x: -10, y: 122, depth: 2_095 },
            ProjectedPoint { x: 6, y: 122, depth: 2_095 },
            ProjectedPoint { x: -9, y: 72, depth: 2_095 },
            ProjectedPoint { x: 6, y: 72, depth: 2_095 },
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
        texture[usize::from(TEXTURE_OFFSET) + 4 * TEXTURE_ROW_STRIDE + 16] =
            PALETTE_INDEX << 4;
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
        assert_eq!(raster.indices()[TARGET[1] * WIDTH + TARGET[0]], PALETTE_INDEX);
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
