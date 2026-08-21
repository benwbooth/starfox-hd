//! Keep the high-level SF1 shipping port on flat typed Rust state.
//!
//! Source-machine details belong in `sf-oracle`; the dedicated `sf-spc`
//! silicon model is outside this gameplay-state boundary.

use std::fs;
use std::path::{Path, PathBuf};

const SHIPPING_CRATES: &[&str] = &[
    "sf-app",
    "sf-audio",
    "sf-core",
    "sf-game",
    "sf-map",
    "sf-path",
    "sf-render",
    "sf-strat",
];

const FORBIDDEN_SUBSTRINGS: &[&str] = &["self.memory", ".memory", "memory:"];

const FORBIDDEN_IDENTIFIERS: &[&str] = &[
    "program_counter",
    "stack_pointer",
    "data_bank_register",
    "direct_page_register",
    "register_a",
    "register_x",
    "register_y",
];

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("shipping source directory") {
            let path = entry.expect("shipping source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    sources
}

#[test]
fn shipping_game_has_no_generic_memory_or_processor_register_state() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust workspace root");
    for crate_name in SHIPPING_CRATES {
        let source_root = workspace.join(crate_name).join("src");
        for path in rust_sources(&source_root) {
            let source = fs::read_to_string(&path).expect("shipping Rust source");
            for forbidden in FORBIDDEN_SUBSTRINGS {
                assert!(
                    !source.contains(forbidden),
                    "shipping source {} contains forbidden architecture term {forbidden:?}",
                    path.display()
                );
            }
            let identifiers: Vec<_> = source
                .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .collect();
            for forbidden in FORBIDDEN_IDENTIFIERS {
                assert!(
                    !identifiers.contains(forbidden),
                    "shipping source {} contains forbidden processor identifier {forbidden:?}",
                    path.display()
                );
            }
        }
    }
}
