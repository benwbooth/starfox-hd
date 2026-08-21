use std::fs;
use std::path::PathBuf;

use sf_core::screen_wipe::ScreenWipeKind::{HorizontalReveal, StarReveal};
use sf_map::catalog::{map_id, opening_wipe_plan, OpeningWipePlan};

const SOURCE_CASES: [(&str, u32, Option<&str>, Option<&str>); 19] = [
    (
        "LEVEL1_1.ASM",
        map_id::M1_1,
        Some("mstarwipe_circle"),
        Some("mscramwipe_circle"),
    ),
    ("LEVEL1_2.ASM", map_id::M1_2, Some("mstarwipe_circle"), None),
    ("LEVEL1_3.ASM", map_id::M1_3, None, None),
    (
        "LEVEL1_4.ASM",
        map_id::M1_4,
        Some("mscramwipe_circle"),
        None,
    ),
    ("LEVEL1_5.ASM", map_id::M1_5, Some("mstarwipe_circle"), None),
    ("LEVEL1_6.ASM", map_id::M1_6, None, None),
    (
        "LEVEL2_1.ASM",
        map_id::M2_1,
        Some("mstarwipe_circle"),
        Some("mscramwipe_circle"),
    ),
    ("LEVEL2_2.ASM", map_id::M2_2, Some("mstarwipe_circle"), None),
    (
        "LEVEL2_3.ASM",
        map_id::M2_3,
        Some("mscramwipe_circle"),
        None,
    ),
    ("LEVEL2_4.ASM", map_id::M2_4, Some("mstarwipe_circle"), None),
    ("LEVEL2_5.ASM", map_id::M2_5, Some("mstarwipe_circle"), None),
    ("LEVEL2_6.ASM", map_id::M2_6, None, None),
    (
        "LEVEL3_1.ASM",
        map_id::M3_1,
        Some("mstarwipe_circle"),
        Some("mscramwipe_circle"),
    ),
    ("LEVEL3_2.ASM", map_id::M3_2, Some("mstarwipe_circle"), None),
    (
        "LEVEL3_3.ASM",
        map_id::M3_3,
        Some("mscramwipe_circle"),
        None,
    ),
    ("LEVEL3_4.ASM", map_id::M3_4, Some("mstarwipe_circle"), None),
    (
        "LEVEL3_5.ASM",
        map_id::M3_5,
        Some("mscramwipe_circle"),
        None,
    ),
    ("LEVEL3_6.ASM", map_id::M3_6, Some("mstarwipe_circle"), None),
    ("LEVEL3_7.ASM", map_id::M3_7, None, None),
];

fn semantic_kind(source_name: Option<&str>) -> Option<sf_core::screen_wipe::ScreenWipeKind> {
    match source_name {
        Some("mstarwipe_circle") => Some(StarReveal),
        Some("mscramwipe_circle") => Some(HorizontalReveal),
        None => None,
        Some(other) => panic!("unexpected source wipe {other}"),
    }
}

#[test]
fn retail_level_catalog_matches_asm_wipe_operands() {
    let source_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/ultrastarfox/SF/MAPS");

    for (file, map, initializer, later_wipe) in SOURCE_CASES {
        let source = fs::read_to_string(source_root.join(file)).expect("retail map source");
        if let Some(name) = initializer {
            assert!(
                source
                    .lines()
                    .any(|line| { line.contains("initlevel") && line.contains(name) }),
                "{file} initializer lost {name}"
            );
        } else {
            let initializer_line = source
                .lines()
                .find(|line| line.contains("initlevel"))
                .expect("initlevel source line");
            assert!(initializer_line.ends_with(",0") || initializer_line.ends_with("\t0"));
        }
        if let Some(name) = later_wipe {
            assert!(source
                .lines()
                .any(|line| line.contains("wipein") && line.contains(name)));
        }

        assert_eq!(
            opening_wipe_plan(map),
            OpeningWipePlan {
                initial: semantic_kind(initializer),
                on_init_black: semantic_kind(later_wipe),
                init_black_calls_before_reveal: u8::from(later_wipe.is_some()),
            },
            "catalog drift for {file}"
        );
    }
}

#[test]
fn training_uses_the_source_star_reveal() {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/ultrastarfox/SF/MAPS/TRAINING.ASM"),
    )
    .expect("training source");
    assert!(source.contains("initlevel\ttraining,mstarwipe_circle"));
    assert_eq!(
        opening_wipe_plan(map_id::TRAINING),
        OpeningWipePlan {
            initial: Some(StarReveal),
            on_init_black: None,
            init_black_calls_before_reveal: 0,
        }
    );
}
