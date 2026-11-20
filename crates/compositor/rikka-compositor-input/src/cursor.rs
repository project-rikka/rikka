use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_types::OutputId;

use crate::route::RouteOutput;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPlacement {
    pub target: OutputId,
    pub local: Point<f64, Logical>,
    pub scale: f64,
}

pub trait CursorSink: Send + 'static {
    fn place(&mut self, placement: CursorPlacement);
}

impl<F: FnMut(CursorPlacement) + Send + 'static> CursorSink for F {
    fn place(&mut self, placement: CursorPlacement) {
        self(placement);
    }
}

pub(crate) struct CursorFollow {
    sink: Option<Box<dyn CursorSink>>,
    pending: Option<CursorPlacement>,
}

impl CursorFollow {
    pub(crate) fn new(sink: Option<Box<dyn CursorSink>>) -> Self {
        Self {
            sink,
            pending: None,
        }
    }

    pub(crate) fn moved(&mut self, position: Point<f64, Logical>, output: &RouteOutput) {
        let Some(target) = output.cursor else {
            return;
        };
        self.pending = Some(CursorPlacement {
            target,
            local: position - output.area.loc.to_f64(),
            scale: output.scale,
        });
    }

    pub(crate) fn flush(&mut self) {
        let Some(placement) = self.pending.take() else {
            return;
        };
        if let Some(sink) = &mut self.sink {
            sink.place(placement);
        }
    }
}

impl std::fmt::Debug for CursorFollow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CursorFollow")
            .field("sink", &self.sink.is_some())
            .field("pending", &self.pending)
            .finish()
    }
}
