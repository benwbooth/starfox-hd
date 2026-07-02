//! Byte-equality of the built path catalog against the C oracle.
//!
//! Fixtures were dumped by a standalone harness compiling
//! `src/path/path_literals.c` directly: `path_blob.bin` is the catalog data,
//! `path_offsets.bin` the little-endian u16 offset table, `path_meta.txt`
//! holds `<blob_len> <offset_count>`.

use sf_path::literals;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn catalog_matches_c_oracle() {
    let blob = fixture("path_blob.bin");
    let offsets_raw = fixture("path_offsets.bin");
    let meta = String::from_utf8(fixture("path_meta.txt")).unwrap();
    let mut meta_it = meta.split_ascii_whitespace();
    let blob_len: usize = meta_it.next().unwrap().parse().unwrap();
    let offset_count: usize = meta_it.next().unwrap().parse().unwrap();
    assert_eq!(blob_len, blob.len(), "fixture self-consistency");
    assert_eq!(offset_count * 2, offsets_raw.len(), "fixture self-consistency");

    let offsets: Vec<u16> = offsets_raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    let catalog = literals::get_catalog();

    assert_eq!(
        catalog.data.len(),
        blob.len(),
        "blob length (rust {} vs C {})",
        catalog.data.len(),
        blob.len()
    );
    if catalog.data != blob {
        let first = catalog
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "blob diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            catalog.data[first], blob[first]
        );
    }

    assert_eq!(catalog.offsets, offsets, "path offset table");
}
