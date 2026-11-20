use std::sync::Arc;
use std::time::Duration;

use rikka_ime::{Caret, Composing, Panel};
use rikka_seam::{Payload, Request, SeamClient, SeamError};
use rikka_session::following;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{DurationToken, State, after_keyed, cancel, motion, spawn, stream};
use shell_kit::{Outputs, Positioner};

use crate::service::{CAPABILITY, CHANGED};

const ADRIFT: (f32, f32) = (0.42, 0.76);

const PARTING: &str = "ime.parting";

#[derive(State)]
pub(crate) struct Window {
    seam: SeamClient,
    panel: Panel,
    shown: Arc<Composing>,
    closing: bool,
    caret: Option<Option<Caret>>,
    outputs: Outputs,
    positioner: Positioner,
}

impl Window {
    pub(crate) fn new(
        seam: SeamClient,
        panel: Panel,
        outputs: Outputs,
        positioner: Positioner,
    ) -> Self {
        let shown = panel.composing();
        Self {
            seam,
            panel,
            shown,
            closing: false,
            caret: None,
            outputs,
            positioner,
        }
    }

    pub(crate) fn start(&mut self) {
        let announced = following::changes(&self.seam, CAPABILITY, CHANGED);
        let moved =
            following::states::<Option<Caret>>(&self.seam, Caret::CAPABILITY, Caret::MOVED, || {
                Request::new(Caret::CAPABILITY, Caret::WHERE).require(Caret::VERSION)
            });
        stream(announced, |window: &mut Window, ()| window.compose());
        stream(moved, |window: &mut Window, at: Option<Caret>| {
            window.caret = Some(at);
            window.aim();
        });
        spawn(
            self.seam
                .call(Request::new(Caret::CAPABILITY, Caret::WHERE).require(Caret::VERSION)),
            |window: &mut Window, reply: Result<Payload, SeamError>| {
                let at = match reply.and_then(|payload| payload.decode()) {
                    Ok(at) => at,
                    Err(error) => {
                        tracing::warn!(
                            "seam call {}/{} failed: {error}",
                            Caret::CAPABILITY,
                            Caret::WHERE
                        );
                        None
                    }
                };
                window.caret = Some(at);
                window.aim();
            },
        );
    }

    pub(crate) fn shown(&self) -> &Composing {
        &self.shown
    }

    pub(crate) fn closing(&self) -> bool {
        self.closing
    }

    pub(crate) fn positioner(&self) -> &Positioner {
        &self.positioner
    }

    pub(crate) fn moved(&mut self) {
        self.aim();
    }

    fn compose(&mut self) {
        let next = self.panel.composing();
        if next.is_empty() && !self.shown.is_empty() {
            self.part();
            return;
        }
        if self.closing {
            cancel(PARTING);
        }
        self.shown = next;
        self.closing = false;
        self.aim();
    }

    fn part(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        let leaving = motion::POP_OUT
            .duration()
            .map_or(Duration::ZERO, DurationToken::duration);
        after_keyed(PARTING, leaving, Window::parted);
    }

    fn parted(&mut self) {
        self.closing = false;
        self.shown = self.panel.composing();
        self.aim();
    }

    fn aim(&self) {
        self.positioner.set_anchor_rect(self.spot());
    }

    fn spot(&self) -> Option<Rect> {
        if self.shown.is_empty() {
            return None;
        }
        let caret = (*self.caret.as_ref()?).map(|caret| caret.rect());
        caret.or(self.shown.spot).or_else(|| {
            let first = self.outputs.all().into_iter().next()?;
            let adrift = Point::new(first.size.width * ADRIFT.0, first.size.height * ADRIFT.1);
            Some(Rect::from_origin_size(first.to_desktop(adrift), Size::ZERO))
        })
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("candidates", &self.shown.candidates.entries.len())
            .field("caret", &self.caret)
            .finish_non_exhaustive()
    }
}
