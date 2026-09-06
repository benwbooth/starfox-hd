//! Native point-block metadata. Geometry remains in `shape_data`; this catalog
//! preserves the encoding and mirrored-pair boundaries that affect rounding.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointFormat {
    Bytes,
    Words,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointBlock {
    /// Provenance only; native transforms do not fetch source instructions.
    pub source_address: u32,
    pub format: PointFormat,
    pub mirrored: bool,
    pub first_vertex: u16,
    /// Number of authored inputs; a mirrored input emits two vertices.
    pub count: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct PointProgram {
    /// Complete animation period, including a single frame for static shapes.
    pub frames: &'static [&'static [PointBlock]],
}

impl PointProgram {
    /// The source frame selector uses only the low six bits of the object word.
    pub fn frame(&self, animation: u16) -> Option<&'static [PointBlock]> {
        if self.frames.is_empty() {
            None
        } else {
            Some(self.frames[usize::from(animation & 63) % self.frames.len()])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_masks_flags_before_wrapping_the_animation_period() {
        const fn block(source_address: u32) -> PointBlock {
            PointBlock {
                source_address,
                format: PointFormat::Bytes,
                mirrored: false,
                first_vertex: 0,
                count: 1,
            }
        }
        static PROGRAM: PointProgram = PointProgram {
            frames: &[&[block(0)], &[block(1)], &[block(2)]],
        };
        assert_eq!(PROGRAM.frame(64).unwrap()[0].source_address, 0);
        assert_eq!(PROGRAM.frame(65).unwrap()[0].source_address, 1);
        assert_eq!(PROGRAM.frame(u16::MAX).unwrap()[0].source_address, 0);
        assert!(PointProgram { frames: &[] }.frame(0).is_none());
    }
}
