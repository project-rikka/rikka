use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use rikka_image_edit::{Imaging, Mark, RemovedMark, Sketch, View};
use rikka_types::{CursorShape, Point, PointerButton, Rect, Size};
use rikka_ui::{Behavior, ComputedImage, Element, Event, EventCtx, ImageSource, PaintCtx, Widget};

use crate::shared::Picked;
use crate::tools::Pick;

const REACH: f32 = 7.0;

pub(super) struct Canvas {
    pub(crate) picture: ImageSource,
    pub(crate) natural: Size,
    pub(super) sketch: Rc<RefCell<Sketch>>,
    pub(crate) picked: Picked,
}

#[derive(Debug, Default)]
pub(crate) struct CanvasState {
    marks: ComputedImage,
    drawing: Option<Mark>,
    rubbing: Option<Vec<RemovedMark>>,
    drawn: Option<Drawn>,
    place: Rect,
    scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Drawn {
    width: u32,
    height: u32,
    revision: u64,
}

impl Canvas {
    fn place(&self, bounds: Rect) -> Rect {
        if self.natural.is_empty() || bounds.size.is_empty() {
            return Rect::ZERO;
        }
        let fit = (bounds.width() / self.natural.width)
            .min(bounds.height() / self.natural.height)
            .min(1.0);
        let size = Size::new(self.natural.width * fit, self.natural.height * fit);
        Rect::from_origin_size(
            Point::new(
                bounds.min_x() + (bounds.width() - size.width) / 2.0,
                bounds.min_y() + (bounds.height() - size.height) / 2.0,
            ),
            size,
        )
    }

    fn at(&self, state: &CanvasState, position: Point) -> Option<Point> {
        state
            .place
            .contains(position)
            .then(|| self.somewhere(state, position))
    }

    fn somewhere(&self, state: &CanvasState, position: Point) -> Point {
        let place = state.place;
        if place.size.is_empty() {
            return Point::ZERO;
        }
        let across = self.natural.width / place.width();
        let down = self.natural.height / place.height();
        Point::new(
            ((position.x - place.min_x()) * across).clamp(0.0, self.natural.width),
            ((position.y - place.min_y()) * down).clamp(0.0, self.natural.height),
        )
    }

    fn reach(&self, state: &CanvasState) -> f32 {
        let place = state.place;
        match place.size.is_empty() {
            true => REACH,
            false => REACH * self.natural.width / place.width(),
        }
    }

    fn layer(&self, state: &CanvasState) -> Option<View> {
        let place = state.place;
        if place.size.is_empty() || self.natural.is_empty() {
            return None;
        }
        let across = (place.width() * state.scale.max(1.0)).round().max(1.0);
        let down = (place.height() * state.scale.max(1.0)).round().max(1.0);
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "a rounded surface extent is a small positive number of pixels"
        )]
        Some(View {
            width: across as u32,
            height: down as u32,
            scale: across / self.natural.width,
        })
    }

    fn draw_marks(&self, state: &mut CanvasState) {
        let Some(view) = self.layer(state) else {
            return;
        };
        let Some(compute) = rikka_picture::SharedGpu::compute(Duration::ZERO) else {
            tracing::warn!("markup requires GPU compute");
            return;
        };
        let imaging = Imaging::shared(&compute);
        let sketch = self.sketch.borrow();
        match imaging.marks_layer(sketch.marks(), state.drawing.as_ref(), view) {
            Ok(layer) => {
                state.marks.show(layer);
                state.drawn = Some(Drawn {
                    width: view.width,
                    height: view.height,
                    revision: sketch.revision(),
                });
            }
            Err(error) => tracing::warn!("failed to draw markup: {error}"),
        }
    }

    fn moved(&self, state: &mut CanvasState, ctx: &mut EventCtx<'_>) {
        state.drawn = None;
        ctx.request_paint();
    }

    fn needs_drawing(&self, state: &CanvasState) -> bool {
        let Some(view) = self.layer(state) else {
            return false;
        };
        state.drawn
            != Some(Drawn {
                width: view.width,
                height: view.height,
                revision: self.sketch.borrow().revision(),
            })
    }

    fn remove_mark_at(&self, state: &mut CanvasState, at: Point) -> bool {
        let reach = self.reach(state);
        let Some(rubbing) = state.rubbing.as_mut() else {
            return false;
        };
        let Some(erased) = self.sketch.borrow_mut().remove_at(at, reach) else {
            return false;
        };
        rubbing.push(erased);
        true
    }

    fn settle(&self, state: &mut CanvasState) -> bool {
        let mut changed = false;
        if let Some(mark) = state.drawing.take()
            && mark.is_significant()
        {
            self.sketch.borrow_mut().push(mark);
            changed = true;
        }
        if let Some(erased) = state.rubbing.take()
            && !erased.is_empty()
        {
            self.sketch.borrow_mut().commit_erase(erased);
            changed = true;
        }
        changed
    }
}

impl Widget for Canvas {
    type State = CanvasState;

    fn paint(&self, state: &mut CanvasState, ctx: &mut PaintCtx<'_>) {
        state.scale = ctx.scale();
        state.place = self.place(ctx.bounds());
        if state.place.size.is_empty() {
            return;
        }
        if self.needs_drawing(state) {
            self.draw_marks(state);
            ctx.request_frame();
        }
        ctx.draw_image(state.place, self.picture.clone());
        if state.marks.size().is_some() {
            ctx.draw_image(state.place, ImageSource::Computed(state.marks.clone()));
        }
    }

    fn event(&self, state: &mut CanvasState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let Some(at) = self.at(state, position) else {
                    return;
                };
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.set_handled();
                match self.picked.pick {
                    Pick::Draw(tool) => {
                        let width = self.picked.width.pixels();
                        state.drawing = Some(Mark::new(tool, self.picked.ink, width, at));
                    }
                    Pick::Erase => {
                        state.rubbing = Some(Vec::new());
                        self.remove_mark_at(state, at);
                    }
                }
                self.moved(state, ctx);
            }
            Event::PointerMoved(position) => {
                if state.drawing.is_none() && state.rubbing.is_none() {
                    return;
                }
                let at = self.somewhere(state, position);
                let changed = match state.drawing.as_mut() {
                    Some(mark) => mark.extend_to(at),
                    None => self.remove_mark_at(state, at),
                };
                if changed {
                    self.moved(state, ctx);
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.drawing.is_none() && state.rubbing.is_none() {
                    return;
                }
                ctx.release_pointer();
                let changed = self.settle(state);
                self.moved(state, ctx);
                if changed {
                    ctx.run(|| {});
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::Crosshair)
    }

    fn relocated(&self, state: &mut CanvasState, by: Point) {
        state.place = state.place.translate(by);
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }
}

impl rikka_ui::View for Canvas {
    fn build(self) -> Element {
        Element::new(self)
    }
}
