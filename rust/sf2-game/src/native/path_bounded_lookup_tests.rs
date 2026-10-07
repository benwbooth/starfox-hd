//! Bank-end lookups (`$7F:A62D`, `$7F:A64F`): in-bank entries are constant
//! source data; any selector past the table faults instead of aliasing the
//! mutable low-bank memory the source would read.
use super::super::path_fields::{ByteField, WordField};
use super::super::render::MaterialSetId;
use super::tests::{setup, world};
use super::*;

const WORDS: &[u16] = &[0x81F4, 0x82FE, 0xFFFF];
const BYTES: &[u8] = &[7, 9, 11, 13];

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: super::super::PathId::from_catalog_index(0),
        command_index,
    }
}

fn run(statement: Statement, selector: u8) -> (Result<(), ProgramError>, ObjectStore, ObjectId) {
    let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().extension.path_state.script_parameter = selector;
    let result = runtime
        .resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1)
        .map(|_| ());
    (result, objects, owner)
}

fn budget_only(result: Result<(), ProgramError>) {
    assert_eq!(
        result,
        Err(ProgramError::BudgetExceeded { cursor: at(1), executed: 1 })
    );
}

#[test]
fn word_lookup_assigns_every_in_bank_entry_and_faults_past_the_table() {
    for selector in 0..=u8::MAX {
        let (result, objects, owner) = run(
            Statement::LookupWordBounded {
                selector: ByteField::ScriptParameter,
                field: WordField::RelativePosition(super::super::path_fields::Axis::X),
                values: WORDS,
                next: at(1),
            },
            selector,
        );
        let actor = objects.get(owner).unwrap();
        match WORDS.get(usize::from(selector)) {
            Some(value) => {
                budget_only(result);
                assert_eq!(actor.extension.relative_position.x, *value as i16);
                assert_eq!(actor.base.path, Some(at(1)));
            }
            None => {
                assert_eq!(
                    result,
                    Err(ProgramError::LookupSelectionOutOfBounds { index: selector, count: WORDS.len() })
                );
                assert_eq!(actor.extension.relative_position.x, 0);
            }
        }
    }
}

#[test]
fn byte_lookup_assigns_every_in_bank_entry_and_faults_past_the_table() {
    for selector in 0..=u8::MAX {
        let (result, objects, owner) = run(
            Statement::LookupByteBounded {
                selector: ByteField::ScriptParameter,
                field: ByteField::Health,
                values: BYTES,
                next: at(1),
            },
            selector,
        );
        let actor = objects.get(owner).unwrap();
        match BYTES.get(usize::from(selector)) {
            Some(value) => {
                budget_only(result);
                assert_eq!(actor.base.hit_points, *value);
            }
            None => {
                assert_eq!(
                    result,
                    Err(ProgramError::LookupSelectionOutOfBounds { index: selector, count: BYTES.len() })
                );
            }
        }
    }
}

#[test]
fn material_lookup_sets_the_material_word_or_faults_without_clearing_it() {
    let materials: Vec<MaterialSetId> = WORDS.iter().map(|w| MaterialSetId::from_catalog_token(*w)).collect();
    let materials: &'static [MaterialSetId] = Box::leak(materials.into_boxed_slice());
    for selector in 0..8u8 {
        let (result, objects, owner) = run(
            Statement::SelectMaterialSet {
                selector: ByteField::ScriptParameter,
                materials,
                next: at(1),
            },
            selector,
        );
        let actor = objects.get(owner).unwrap();
        match materials.get(usize::from(selector)) {
            Some(material) => {
                budget_only(result);
                assert_eq!(actor.extension.material_set, Some(*material));
            }
            None => {
                assert!(matches!(result, Err(ProgramError::LookupSelectionOutOfBounds { .. })));
                assert_eq!(actor.extension.material_set, None);
            }
        }
    }
}
