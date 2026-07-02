//! Shape-table integrity checks. The generated Rust tables in
//! `sf-render/src/shape_data.rs` are emitted by `tools/shape_compiler.py`.
//!
//! (The former `shape_data_matches_c_header` cross-check compared these
//! tables against the generated C header `src/renderer/shape_data.h`. That
//! C tree — and its header oracle — has been removed, so the check is gone;
//! `shape_compiler.py` now emits the Rust table as the single source.)

use sf_render::shape_data::SHAPE_DATA;

#[test]
fn wireframe_segments_present() {
    // Face2 wireframe shapes (op_0 runway rails, shyper ring) must keep
    // their num_verts == 2 line segments.
    for id in [508u16, 268u16] {
        let entry = SHAPE_DATA
            .iter()
            .find(|e| e.shape_id == id)
            .unwrap_or_else(|| panic!("shape {id} missing"));
        assert!(
            entry.faces.iter().any(|f| f.num_verts == 2),
            "shape {id} should contain Face2 line segments"
        );
    }
}
