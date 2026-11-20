use crate::renderer::Renderer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameWork {
    pub frames: u64,
    pub walked: u64,
    pub emitted: u64,
    pub reused: u64,
    pub restructured: u64,
    pub uploaded_instances: u64,
    pub uploaded_bytes: u64,
}

impl FrameWork {
    #[must_use]
    pub fn since(self, earlier: FrameWork) -> FrameWork {
        FrameWork {
            frames: self.frames.saturating_sub(earlier.frames),
            walked: self.walked.saturating_sub(earlier.walked),
            emitted: self.emitted.saturating_sub(earlier.emitted),
            reused: self.reused.saturating_sub(earlier.reused),
            restructured: self.restructured.saturating_sub(earlier.restructured),
            uploaded_instances: self
                .uploaded_instances
                .saturating_sub(earlier.uploaded_instances),
            uploaded_bytes: self.uploaded_bytes.saturating_sub(earlier.uploaded_bytes),
        }
    }

    pub fn add(&mut self, frame: FrameWork) {
        self.frames += frame.frames;
        self.walked += frame.walked;
        self.emitted += frame.emitted;
        self.reused += frame.reused;
        self.restructured += frame.restructured;
        self.uploaded_instances += frame.uploaded_instances;
        self.uploaded_bytes += frame.uploaded_bytes;
    }
}

impl Renderer {
    #[must_use]
    pub fn frame_work(&self) -> FrameWork {
        self.painter.work
    }
}
