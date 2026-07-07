//! Byte-equality of the built path catalog against the blessed snapshot.
//!
//! The fixtures were originally dumped from the C builder
//! (`src/path/path_literals.c`); the Rust catalog has since diverged from C
//! ON PURPOSE where the 65816 oracle proved the C encoding wrong vs the ROM
//! blob (P_ADDW for out-of-range world adds, P_SPAWN* coord/4 payloads,
//! p_sound2 for the ASM P_SOUND macro, P_SET 0 -> P_ZERO — see
//! sf-oracle/tests/audit_path.rs). The fixtures are now a snapshot of the
//! ROM-corrected Rust output; re-bless with SF_BLESS_FIXTURES=1 after
//! intentional data changes.

use sf_path::literals;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> Vec<u8> {
    let path = fixture_path(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn catalog_matches_c_oracle() {
    if std::env::var("SF_BLESS_FIXTURES").is_ok() {
        let catalog = literals::get_catalog();
        let offsets_raw: Vec<u8> =
            catalog.offsets.iter().flat_map(|o| o.to_le_bytes()).collect();
        std::fs::write(fixture_path("path_blob.bin"), &catalog.data).unwrap();
        std::fs::write(fixture_path("path_offsets.bin"), &offsets_raw).unwrap();
        std::fs::write(
            fixture_path("path_meta.txt"),
            format!("{} {}\n", catalog.data.len(), catalog.offsets.len()),
        )
        .unwrap();
        eprintln!("blessed path catalog fixtures");
        return;
    }
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
