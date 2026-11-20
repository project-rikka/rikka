mod service;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::Caret as Composing;
use rikka_ime::{Caret, PanelAnchor};
use rikka_seam::Publisher;

use crate::context::{InputCtx, InputRef, input_ref};
use crate::outputs::Outputs;
use service::Aims;
pub(crate) use service::CaretService;

#[derive(Debug)]
pub(crate) struct Carets {
    composing: Option<Composing>,
    aims: Aims,
}

impl Default for Carets {
    fn default() -> Self {
        Self {
            composing: None,
            aims: Aims::new(Caret::MOVED),
        }
    }
}

impl Carets {
    pub(crate) fn attach(&mut self, publisher: Publisher) {
        self.aims.attach(publisher);
    }

    pub(crate) fn follow(&mut self, composing: Option<Composing>) {
        self.composing = composing;
    }
}

impl InputCtx<'_> {
    pub(crate) fn announce_caret(&mut self) {
        let at = input_ref!(self).caret();
        self.input.caret.aims.publish(at);
    }
}

impl InputRef<'_> {
    fn caret(&self) -> Option<Caret> {
        let composing = self.input.caret.composing?;
        let at = self.surface_origin(composing.surface)? + composing.rect.loc;
        Some(Caret {
            x: at.x,
            y: at.y,
            width: composing.rect.size.w,
            height: composing.rect.size.h,
        })
    }

    pub(crate) fn aim(&self) -> Option<PanelAnchor> {
        if let Some(caret) = self.caret()
            && let Some(output) = self.outputs.output_named_under(Point::new(
                f64::from(caret.x) + f64::from(caret.width) / 2.0,
                f64::from(caret.y) + f64::from(caret.height) / 2.0,
            ))
        {
            return Some(PanelAnchor::Caret { output, caret });
        }
        let pointer = self.cursor.position();
        let output = self.outputs.output_named_under(pointer)?;
        Some(PanelAnchor::Pointer {
            output,
            x: pointer.x.round() as i32,
            y: pointer.y.round() as i32,
        })
    }
}

impl Outputs {
    fn output_named_under(&self, point: Point<f64, Logical>) -> Option<String> {
        self.containing(point)
            .map(|output| output.name().to_owned())
    }
}
