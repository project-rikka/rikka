use rikka_compositor_surface::{Feedback, Presentation, PresentationKind};

#[must_use = "unused presentation feedback is discarded"]
#[derive(Debug, Default)]
pub(crate) struct Presenting {
    composited: Feedback,
    scanned: Feedback,
}

impl Presenting {
    pub(super) fn new(composited: Feedback, scanned: Feedback) -> Self {
        Self {
            composited,
            scanned,
        }
    }

    pub(super) fn feedbacks(self, presentation: &Presentation) -> [(Feedback, Presentation); 2] {
        let composited = Presentation {
            flags: presentation.flags.difference(PresentationKind::ZERO_COPY),
            ..*presentation
        };
        [(self.composited, composited), (self.scanned, *presentation)]
    }
}
