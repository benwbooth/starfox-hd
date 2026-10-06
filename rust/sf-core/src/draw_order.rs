//! Shared source painter ordering for scene simulation and rendering.
use crate::{sf1_shape_metrics::sf1_shape_metrics, snes_trig, DrawListEntry};

pub fn view_position(entry: &DrawListEntry, camera: [i16; 3], matrix: [[i16; 3]; 3]) -> [i16; 3] {
    let relative = [entry.x, entry.y, entry.z].map(|coordinate| (coordinate >> 16) as i16);
    let relative = std::array::from_fn::<_, 3, _>(|axis| relative[axis].wrapping_sub(camera[axis]));
    let (x, y, z) = snes_trig::matrix_rotate_q15(matrix, relative[0], relative[1], relative[2]);
    [x, y, z]
}

pub fn source_painter_order(
    entries: &[DrawListEntry],
    camera: [i16; 3],
    rotation: [u16; 3],
) -> Vec<usize> {
    let matrix = snes_trig::zxy_matrix_q15_fine(rotation[0], rotation[1], rotation[2]);
    let mut order: Vec<_> = (0..entries.len()).collect();
    // `mallrotzsort` inserts farthest first, with later equal-depth entries
    // preceding earlier ones. Shape bias and ground placement wrap as words.
    order.sort_by_key(|&index| {
        let entry = &entries[index];
        let bias = sf1_shape_metrics(entry.shape_id).map_or(0, |metrics| metrics.sort_depth);
        let depth = view_position(entry, camera, matrix)[2]
            .wrapping_add(entry.sort_z)
            .wrapping_add(bias);
        (std::cmp::Reverse(depth), std::cmp::Reverse(index))
    });
    order
}
