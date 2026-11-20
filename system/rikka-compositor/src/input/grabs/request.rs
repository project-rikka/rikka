use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_types::OutputId;

use rikka_compositor_tiling::Edges;

use crate::context::{InputCtx, windows_ctx};
use crate::windows::Anchor;

#[derive(Debug)]
pub(crate) enum GrabRequest {
    Moved {
        window: WindowId,
        location: Point<i32, Logical>,
    },
    Resized {
        window: WindowId,
        rect: Rect<i32, Logical>,
        anchor: Anchor,
    },
    Retiled {
        output: OutputId,
        window: WindowId,
        edges: Edges,
        delta: Point<f64, Logical>,
    },
    Divided(Vec<(WindowId, Rect<i32, Logical>)>),
}

impl GrabRequest {
    fn absorb(&mut self, next: Self) {
        if let (
            Self::Retiled {
                window,
                edges,
                delta,
                ..
            },
            Self::Retiled {
                window: next_window,
                edges: next_edges,
                delta: step,
                ..
            },
        ) = (&mut *self, &next)
            && window == next_window
            && edges == next_edges
        {
            *delta += *step;
            return;
        }
        *self = next;
    }
}

#[derive(Debug, Default)]
pub(crate) struct GrabRequests {
    pending: Option<GrabRequest>,
    active: bool,
}

impl GrabRequests {
    pub(crate) fn begin(&mut self) {
        self.active = true;
    }

    pub(crate) fn end(&mut self) {
        self.active = false;
        self.pending = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn push(&mut self, request: GrabRequest) {
        match &mut self.pending {
            Some(pending) => pending.absorb(request),
            None => self.pending = Some(request),
        }
    }

    fn take(&mut self) -> Option<GrabRequest> {
        self.pending.take()
    }
}

impl InputCtx<'_> {
    pub(crate) fn apply_grab_requests(&mut self) {
        let Some(request) = self.input.grabs.requests.take() else {
            return;
        };
        match request {
            GrabRequest::Moved { window, location } => {
                windows_ctx!(self).move_window(window, location);
                self.windows.session_changed();
            }
            GrabRequest::Resized {
                window,
                rect,
                anchor,
            } => windows_ctx!(self).ask_size(window, rect, anchor),
            GrabRequest::Retiled {
                output,
                window,
                edges,
                delta,
            } => windows_ctx!(self).resize_tile(output, window, edges, delta),
            GrabRequest::Divided(moved) => windows_ctx!(self).divide(moved),
        }
    }
}
