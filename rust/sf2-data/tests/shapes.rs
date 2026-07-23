//! Integrity checks for the generated, flat SF2 shape catalog.

use sf2_data::shape_data::SHAPE_DATA;

#[test]
fn visibility_triangles_and_animation_frames_are_in_bounds() {
    let mut one_sided_faces = 0usize;
    let mut two_sided_faces = 0usize;

    for entry in &SHAPE_DATA {
        for frame in entry.animation_frames {
            assert_eq!(
                frame.len(),
                entry.vertices.len(),
                "shape {} animation vertex count",
                entry.header_index
            );
        }

        for face in entry.faces {
            match face.visibility_vertices {
                Some(indices) => {
                    one_sided_faces += 1;
                    for index in indices {
                        assert!(
                            usize::from(index) < entry.vertices.len(),
                            "shape {} visibility vertex {} is out of bounds",
                            entry.header_index,
                            index
                        );
                    }
                }
                None => two_sided_faces += 1,
            }
        }
    }

    assert!(one_sided_faces > 9_000, "one-sided face metadata collapsed");
    assert!(two_sided_faces > 500, "two-sided face metadata collapsed");
}
