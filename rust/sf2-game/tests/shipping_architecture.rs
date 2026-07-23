//! Guard the architectural boundary requested for the shipping SF2 port.
//! The byte-addressed compatibility host remains available to `sf-oracle`
//! behind `oracle-bridge`; default native gameplay must never grow equivalent
//! state or machine-oriented accessors.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_NATIVE_TERMS: &[&str] = &[
    "self.memory",
    ".memory",
    "memory:",
    "work_ram",
    "wram",
    "main_state",
    "read_byte(",
    "write_byte(",
    "read_word(",
    "write_word(",
    "cpu_bridge",
    "cpu.",
    "cpu_",
    "w65c816",
    "registers",
    "registers.",
    "program_counter",
    "stack_pointer",
    "data_bank",
    "direct_page",
];

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("native source directory") {
            let path = entry.expect("native source entry").path();
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
fn default_native_game_has_no_byte_addressed_or_processor_state() {
    let native = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native");
    for path in rust_sources(&native) {
        let source = fs::read_to_string(&path).expect("native Rust source");
        for forbidden in FORBIDDEN_NATIVE_TERMS {
            assert!(
                !source.contains(forbidden),
                "shipping source {} contains forbidden architecture term {forbidden:?}",
                path.display()
            );
        }
    }
}

#[test]
fn compatibility_host_is_oracle_feature_gated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let library = fs::read_to_string(manifest.join("src/lib.rs")).expect("sf2-game library");
    for module_path in [
        "cpu_bridge.rs",
        "map_host.rs",
        "memory.rs",
        "object.rs",
        "path_host.rs",
        "strategy.rs",
    ] {
        let marker = format!("#[cfg(feature = \"oracle-bridge\")]\n#[path = \"{module_path}\"]");
        assert!(
            library.contains(&marker),
            "compatibility module {module_path} escaped the oracle feature gate"
        );
    }
}
