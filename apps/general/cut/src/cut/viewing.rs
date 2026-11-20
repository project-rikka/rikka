use std::time::{Duration, Instant};

use cut_project::{Appearance, Camera, ClipId, Edit, Redraw};

use super::Cut;

const DOLLY_PAUSE: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Dollying {
    camera: ClipId,
    revision: u64,
    at: Instant,
}

impl Cut {
    pub(crate) fn dolly(&mut self, camera: ClipId, framed: Camera) {
        self.live.settle();
        let now = Instant::now();
        let edit = Edit::Redraw {
            changes: vec![(camera, Appearance::Camera(framed))],
            by: Redraw::Move,
            at: self.transport.frame(),
        };
        let revision = self.document.revision();
        let continuing = self.dollying.is_some_and(|dolly| {
            dolly.camera == camera
                && dolly.revision == revision
                && now.duration_since(dolly.at) < DOLLY_PAUSE
        });
        match continuing {
            true => self.amend(edit),
            false => self.edit(edit),
        }
        self.dollying = Some(Dollying {
            camera,
            revision: self.document.revision(),
            at: now,
        });
    }
}
