//! Guard the shared strategy ABI against the authoritative assembler table.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn canonical_rows() -> HashMap<String, usize> {
    let path = repo_root().join("reference/ultrastarfox/SF/STRAT/ISTRATS.ASM");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut rows = HashMap::new();
    for raw in source.lines() {
        let code = raw.split(';').next().unwrap_or("").trim();
        let lower = code.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("def_istrat") else {
            continue;
        };
        let Some(name) = rest
            .trim_start()
            .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
            .next()
        else {
            continue;
        };
        if name != "macro" {
            let row = rows.len();
            assert!(
                rows.insert(name.to_owned(), row).is_none(),
                "duplicate {name}"
            );
        }
    }
    assert_eq!(rows.len(), 246, "unexpected ISTRATS.ASM row count");
    rows
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut out = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out
}

fn canonical_name(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "lochness" => "lochnessmonster".to_owned(),
        "boss_7" => "boss7".to_owned(),
        other => other.to_owned(),
    }
}

#[test]
fn every_production_is_constant_matches_the_assembler_row() {
    let rows = canonical_rows();
    let root = repo_root();
    let mut checked = 0;
    for crate_name in ["sf-map", "sf-strat", "sf-game", "sf-path"] {
        for path in rust_sources(&root.join("rust").join(crate_name).join("src")) {
            let source = fs::read_to_string(&path).unwrap();
            for (line_no, line) in source.lines().enumerate() {
                let Some(start) = line.find("const IS_") else {
                    continue;
                };
                let rest = &line[start + "const IS_".len()..];
                let Some(colon) = rest.find(':') else {
                    continue;
                };
                let name = &rest[..colon];
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                let Some(end) = rest[eq + 1..].find(';') else {
                    continue;
                };
                let value_text = rest[eq + 1..eq + 1 + end].trim();
                if !value_text.bytes().all(|byte| byte.is_ascii_digit()) {
                    continue;
                }
                let value: usize = value_text.parse().unwrap();
                let key = canonical_name(name);
                let expected = rows.get(&key).unwrap_or_else(|| {
                    panic!(
                        "{}:{}: IS_{name} is not an ISTRATS.ASM row; use a STRAT_ADDR_* constant for non-table labels",
                        path.display(),
                        line_no + 1,
                    )
                });
                assert_eq!(
                    value,
                    *expected,
                    "{}:{}: IS_{name} must equal ISTRATS.ASM row {expected}",
                    path.display(),
                    line_no + 1,
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 300, "only checked {checked} strategy constants");
}

#[test]
fn shared_map_constants_match_the_assembler_rows() {
    use sf_map::consts::{is, MAP_ISTRAT_SPACEBAR, MAP_ISTRAT_SPACEBAR1};

    let rows = canonical_rows();
    let row = |name: &str| *rows.get(&name.to_ascii_lowercase()).unwrap() as u32;
    for (name, got) in [
        ("gnd", is::GND),
        ("clshipgnda", is::CLSHIPGNDA),
        ("gate", is::GATE),
        ("flypillars", is::FLYPILLARS),
        ("amoeba", is::AMOEBA),
        ("uperm", is::UPERM),
        ("mine2", is::MINE2),
        ("break_meteor", is::BREAK_METEOR),
        ("hard", is::HARD),
        ("pathdha", is::PATHDHA),
    ] {
        assert_eq!(got, row(name), "sf_map::consts::is::{name}");
    }
    assert_eq!(MAP_ISTRAT_SPACEBAR, row("spacebar"));
    assert_eq!(MAP_ISTRAT_SPACEBAR1, row("spacebar1"));
}
