use rikka_compositor_kms::{CrtcId, Framebuffer, Present, Rect};
use rikka_renderer::DmabufFence;

pub(super) struct Rendered {
    pub(super) framebuffer: Framebuffer,
    pub(super) damage: Vec<Rect>,
    pub(super) fence: Option<DmabufFence>,
    pub(super) submitted: u64,
}

impl Rendered {
    pub(super) fn into_present(self, crtc: CrtcId, frame: u64) -> (Present, u64) {
        let present = Present::new(crtc, frame, self.framebuffer).damage(self.damage);
        let present = match self.fence {
            Some(DmabufFence::SyncFile(sync_file)) => present.fence(sync_file),
            Some(DmabufFence::Readable(readable)) => present.ready_when_readable(readable),
            None => present,
        };
        (present, self.submitted)
    }
}
