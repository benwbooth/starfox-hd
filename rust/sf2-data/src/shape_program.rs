//! Authored face-program graph, separate from the renderer's geometry union.
//! These are decoded data commands, not GSU instructions. Source addresses
//! retain extraction provenance; graph consumers use node indices.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceGroup {
    pub depth_point: u8,
    pub root: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceCommand {
    Visibility {
        triangles: &'static [[u8; 3]],
        next: NodeId,
    },
    BeginBsp {
        root: NodeId,
    },
    /// Spatial children are visited in visibility-dependent order. The
    /// coplanar list is submitted only for the source's negative test result.
    Bsp {
        visibility: u8,
        coplanar: NodeId,
        left: NodeId,
        right: Option<NodeId>,
    },
    /// Submit a face list to the BSP queue, then return to the spatial parent.
    BspLeaf {
        faces: NodeId,
    },
    ReturnBsp,
    Faces {
        /// Range in the corresponding ShapeDataEntry's face union.
        first: u16,
        count: u16,
        /// None is the face-list quit terminator; Some is continuing $FE.
        next: Option<NodeId>,
    },
    Groups {
        entries: &'static [FaceGroup],
    },
    Sprite {
        parameters: [u8; 3],
        next: NodeId,
    },
    VisibleSprite {
        parameters: [u8; 4],
        next: NodeId,
    },
    ClipPlane {
        /// Index into the corresponding ShapeDataEntry's clipping planes.
        plane: u16,
        next: NodeId,
    },
    Quit,
    EndShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceNode {
    pub source_address: u32,
    pub command: FaceCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceProgram {
    pub root: Option<NodeId>,
    pub nodes: &'static [FaceNode],
}

impl FaceProgram {
    pub fn node(&self, id: NodeId) -> Option<&FaceNode> {
        self.nodes.get(usize::from(id.0))
    }
}
