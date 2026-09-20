use super::super::path_fields::{Axis, WordField};
use super::super::PathId;
use super::tests::{setup, world};
use super::*;

#[test]
fn scene_height_addition_samples_live_word_wraps_and_preserves_actor_and_shared_state() {
    let next = PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: 1,
    };
    for destination in [
        WordField::Position(Axis::Y),
        WordField::RelativePosition(Axis::Y),
        WordField::MotionPhase,
    ] {
        let catalog = PathCatalog::new(vec![vec![Statement::AddSceneHeightOffset {
            destination,
            next,
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let entry = objects.get(owner).unwrap().base.path;
        let before_random = random;
        runtime.branch.invert_next = true;
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingSceneHeightOffset)
        );
        assert_eq!(objects, before);
        for offset in 0..=u16::MAX {
            for initial in [0, 32767, 32768, 65535] {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = entry;
                actor.base.wait_timer = 73;
                destination.write(actor, initial);
                let mut expected = actor.clone();
                destination.write(
                    &mut expected,
                    ((u32::from(initial) + u32::from(offset)) % 65536) as u16,
                );
                expected.base.path = Some(next);
                let mut inputs = world(&mut random);
                inputs.scene.height_offset = Some(offset as i16);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: next,
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner), Some(&expected));
                assert!(runtime.branch.invert_next);
                assert_eq!(inputs.scene.height_offset, Some(offset as i16));
            }
        }
        assert_eq!(random, before_random);
    }
}
