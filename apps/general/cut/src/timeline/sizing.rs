use cut_project::Height;
use rikka_app::prelude::*;

use crate::cut::Cut;

pub(crate) const GRIP: f32 = 6.0;
const LINE: f32 = 2.0;

#[derive(Debug)]
pub(crate) struct Sizing {
    pub(crate) layer: usize,
    pub(crate) height: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Pulling {
    from: Option<(f32, f32)>,
}

impl Sizing {
    fn height_at(&self, state: &Pulling, y: f32) -> Option<Height> {
        state
            .from
            .map(|(pressed, height)| Height::new(height + y - pressed))
    }
}

impl Widget for Sizing {
    type State = Pulling;

    fn paint(&self, state: &mut Pulling, ctx: &mut PaintCtx<'_>) {
        if !ctx.hovered() && state.from.is_none() {
            return;
        }
        let bounds = ctx.bounds();
        ctx.draw_quad(
            Quad::new(Rect::new(
                bounds.x(),
                bounds.max_y() - LINE,
                bounds.width(),
                LINE,
            ))
            .fill(palette().accent),
        );
    }

    fn event(&self, state: &mut Pulling, event: &Event, ctx: &mut EventCtx<'_>) {
        let layer = self.layer;
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.from = Some((position.y, self.height));
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                ctx.request_paint();
            }
            Event::PointerMoved(position) if ctx.captured() => {
                ctx.set_handled();
                if let Some(height) = self.height_at(state, position.y) {
                    ctx.run(move |cut: &mut Cut| cut.pull_layer(layer, height));
                }
            }
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } if ctx.captured() => {
                let height = self.height_at(state, position.y);
                state.from = None;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                if let Some(height) = height {
                    ctx.run(move |cut: &mut Cut| cut.resize_layer(layer, height));
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::ResizeRow)
    }

    fn behavior(&self, _state: &Pulling) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Sizing) -> bool {
        self.layer == other.layer && self.height == other.height
    }
}

impl View for Sizing {
    fn build(self) -> Element {
        Element::new(self).height(GRIP)
    }
}
