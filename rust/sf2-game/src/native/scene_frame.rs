//! Ordinary scene-frame buffer ownership, independent of display timing.
//!
//! The main loop (03:8004) queues one bitmap upload and starts one draw, then
//! joins both before servicing pending scene loads (03:820C). Upload completion
//! (7F:0747) and draw completion (7F:7B57) change different owners. Either may
//! finish first; the next frame cannot queue another upload until the previous
//! one has completed. The alternate-view/partial-upload path is not this owner.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapBuffer {
    First,
    Second,
}

impl BitmapBuffer {
    const fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalFrameWork {
    pub draw_target: BitmapBuffer,
    pub upload_source: BitmapBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOwnershipError {
    PreviousWorkPending,
    NoDrawPending,
    NoUploadPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalFrameBuffers {
    draw_target: BitmapBuffer,
    upload_source: BitmapBuffer,
    drawing: bool,
    uploading: bool,
}

impl Default for NormalFrameBuffers {
    fn default() -> Self {
        Self::new(BitmapBuffer::First, BitmapBuffer::Second)
    }
}

impl NormalFrameBuffers {
    /// Retain explicit roles across scene entry. Boot uses the default pair;
    /// later scene requests must not reset them to manufacture load readiness.
    pub const fn new(draw_target: BitmapBuffer, upload_source: BitmapBuffer) -> Self {
        Self {
            draw_target,
            upload_source,
            drawing: false,
            uploading: false,
        }
    }

    pub const fn next_work(&self) -> NormalFrameWork {
        NormalFrameWork {
            draw_target: self.draw_target,
            upload_source: self.upload_source,
        }
    }

    pub const fn work_pending(&self) -> bool {
        self.drawing || self.uploading
    }

    pub fn begin_frame(&mut self) -> Result<NormalFrameWork, FrameOwnershipError> {
        if self.work_pending() {
            return Err(FrameOwnershipError::PreviousWorkPending);
        }
        self.drawing = true;
        self.uploading = true;
        Ok(self.next_work())
    }

    pub fn finish_draw(&mut self) -> Result<BitmapBuffer, FrameOwnershipError> {
        if !self.drawing {
            return Err(FrameOwnershipError::NoDrawPending);
        }
        let completed = self.draw_target;
        self.draw_target = completed.other();
        self.drawing = false;
        Ok(completed)
    }

    pub fn finish_upload(&mut self) -> Result<BitmapBuffer, FrameOwnershipError> {
        if !self.uploading {
            return Err(FrameOwnershipError::NoUploadPending);
        }
        let completed = self.upload_source;
        self.upload_source = completed.other();
        self.uploading = false;
        Ok(completed)
    }

    /// The scene-load barrier has joined drawing and uploading. Readiness is
    /// the *next upload source*, never the current draw target or scene age.
    pub const fn ready_for_scene_load(&self) -> bool {
        !self.work_pending() && matches!(self.upload_source, BitmapBuffer::Second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_completion_orders_preserve_independent_roles() {
        for draw in [BitmapBuffer::First, BitmapBuffer::Second] {
            for upload in [BitmapBuffer::First, BitmapBuffer::Second] {
                for upload_first in [false, true] {
                    let mut buffers = NormalFrameBuffers::new(draw, upload);
                    assert_eq!(
                        buffers.begin_frame().unwrap(),
                        NormalFrameWork {
                            draw_target: draw,
                            upload_source: upload,
                        }
                    );
                    let busy = buffers;
                    assert_eq!(
                        buffers.begin_frame(),
                        Err(FrameOwnershipError::PreviousWorkPending)
                    );
                    assert_eq!(buffers, busy);
                    assert!(!buffers.ready_for_scene_load());
                    if upload_first {
                        assert_eq!(buffers.finish_upload(), Ok(upload));
                        assert_eq!(buffers.next_work().draw_target, draw);
                        assert!(!buffers.ready_for_scene_load());
                        assert_eq!(buffers.finish_draw(), Ok(draw));
                    } else {
                        assert_eq!(buffers.finish_draw(), Ok(draw));
                        assert_eq!(buffers.next_work().upload_source, upload);
                        assert!(!buffers.ready_for_scene_load());
                        assert_eq!(buffers.finish_upload(), Ok(upload));
                    }
                    assert_eq!(
                        buffers.next_work(),
                        NormalFrameWork {
                            draw_target: draw.other(),
                            upload_source: upload.other(),
                        }
                    );
                    assert_eq!(
                        buffers.ready_for_scene_load(),
                        upload == BitmapBuffer::First
                    );
                    let completed = buffers;
                    assert_eq!(
                        buffers.finish_draw(),
                        Err(FrameOwnershipError::NoDrawPending)
                    );
                    assert_eq!(
                        buffers.finish_upload(),
                        Err(FrameOwnershipError::NoUploadPending)
                    );
                    assert_eq!(buffers, completed);
                }
            }
        }
    }
}
