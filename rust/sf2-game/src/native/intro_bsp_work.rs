//! Native visibility-dependent BSP submission, before polygon rasterization.
//! Visibility flags come from the shape's transformed triangle tests. This
//! pass preserves source ordering and records branch work, not elapsed clocks.

use sf2_data::shape_program::{FaceCommand, FaceProgram, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BspBranchWork {
    pub negative: bool,
    pub has_right_child: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BspSubmission {
    pub face_lists: Vec<NodeId>,
    pub branches: Vec<BspBranchWork>,
    pub leaves: u32,
    pub returns: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BspSubmissionError {
    MissingNode(NodeId),
    MissingVisibility(u8),
    UnexpectedCommand(NodeId),
    Cycle(NodeId),
}

/// Traverse a decoded BSP root with the source's visibility sign bits.
/// A negative test visits left, submits coplanar faces, then visits right.
/// A nonnegative test visits right then left without submitting the coplanar
/// list. A leaf submits its list and returns; it does not execute that list.
pub fn submit_bsp(
    program: &FaceProgram,
    root: NodeId,
    negative_visibility: &[bool],
) -> Result<BspSubmission, BspSubmissionError> {
    enum Task {
        Visit(NodeId),
        Submit(NodeId),
        Leave(NodeId),
    }
    let mut result = BspSubmission::default();
    let mut pending = vec![Task::Visit(root)];
    let mut active = vec![false; program.nodes.len()];
    while let Some(task) = pending.pop() {
        let id = match task {
            Task::Submit(id) => {
                program
                    .node(id)
                    .ok_or(BspSubmissionError::MissingNode(id))?;
                result.face_lists.push(id);
                continue;
            }
            Task::Leave(id) => {
                active[usize::from(id.0)] = false;
                continue;
            }
            Task::Visit(id) => id,
        };
        let node = program
            .node(id)
            .ok_or(BspSubmissionError::MissingNode(id))?;
        if active[usize::from(id.0)] {
            return Err(BspSubmissionError::Cycle(id));
        }
        active[usize::from(id.0)] = true;
        pending.push(Task::Leave(id));
        match node.command {
            FaceCommand::BeginBsp { root } => pending.push(Task::Visit(root)),
            FaceCommand::Bsp {
                visibility,
                coplanar,
                left,
                right,
            } => {
                let negative = *negative_visibility
                    .get(usize::from(visibility))
                    .ok_or(BspSubmissionError::MissingVisibility(visibility))?;
                result.branches.push(BspBranchWork {
                    negative,
                    has_right_child: right.is_some(),
                });
                if negative {
                    if let Some(right) = right {
                        pending.push(Task::Visit(right));
                    }
                    pending.push(Task::Submit(coplanar));
                    pending.push(Task::Visit(left));
                } else {
                    pending.push(Task::Visit(left));
                    if let Some(right) = right {
                        pending.push(Task::Visit(right));
                    }
                }
            }
            FaceCommand::BspLeaf { faces } => {
                result.leaves += 1;
                pending.push(Task::Submit(faces));
            }
            FaceCommand::ReturnBsp => result.returns += 1,
            _ => return Err(BspSubmissionError::UnexpectedCommand(id)),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf2_data::shape_program::FaceNode;

    const fn node(command: FaceCommand) -> FaceNode {
        FaceNode {
            source_address: 0,
            command,
        }
    }

    static NODES: [FaceNode; 6] = [
        node(FaceCommand::Bsp {
            visibility: 0,
            coplanar: NodeId(3),
            left: NodeId(1),
            right: Some(NodeId(2)),
        }),
        node(FaceCommand::BspLeaf { faces: NodeId(4) }),
        node(FaceCommand::BspLeaf { faces: NodeId(5) }),
        node(FaceCommand::Quit),
        node(FaceCommand::Quit),
        node(FaceCommand::Quit),
    ];

    #[test]
    fn visibility_controls_order_and_coplanar_submission() {
        let program = FaceProgram {
            root: Some(NodeId(0)),
            nodes: &NODES,
        };
        let negative = submit_bsp(&program, NodeId(0), &[true]).unwrap();
        assert_eq!(negative.face_lists, [NodeId(4), NodeId(3), NodeId(5)]);
        assert_eq!(negative.leaves, 2);
        assert_eq!(
            negative.branches,
            [BspBranchWork {
                negative: true,
                has_right_child: true
            }]
        );
        let positive = submit_bsp(&program, NodeId(0), &[false]).unwrap();
        assert_eq!(positive.face_lists, [NodeId(5), NodeId(4)]);
        assert_eq!(
            submit_bsp(&program, NodeId(0), &[]),
            Err(BspSubmissionError::MissingVisibility(0))
        );
        assert_eq!(
            submit_bsp(&program, NodeId(6), &[]),
            Err(BspSubmissionError::MissingNode(NodeId(6)))
        );
    }

    #[test]
    fn cycles_fail_but_shared_children_are_visited_again() {
        static SHARED: [FaceNode; 3] = [
            node(FaceCommand::Bsp {
                visibility: 0,
                coplanar: NodeId(2),
                left: NodeId(1),
                right: Some(NodeId(1)),
            }),
            node(FaceCommand::BspLeaf { faces: NodeId(2) }),
            node(FaceCommand::Quit),
        ];
        let program = FaceProgram {
            root: Some(NodeId(0)),
            nodes: &SHARED,
        };
        assert_eq!(
            submit_bsp(&program, NodeId(0), &[false])
                .unwrap()
                .face_lists,
            [NodeId(2), NodeId(2)]
        );
        static CYCLE: [FaceNode; 1] = [node(FaceCommand::BeginBsp { root: NodeId(0) })];
        let program = FaceProgram {
            root: Some(NodeId(0)),
            nodes: &CYCLE,
        };
        assert_eq!(
            submit_bsp(&program, NodeId(0), &[]),
            Err(BspSubmissionError::Cycle(NodeId(0)))
        );
    }
}
