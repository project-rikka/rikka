use rikka_app::prelude::*;
use rikka_figure::{Keys, constrain_angle};
use rikka_image_edit::{Erasing, Ink, View as MarksView};

use super::overlay::{Placing, Shown};
use super::{CHECKER_SQUARE, Canvas, CanvasState, Drag};
use crate::checker;
use crate::colors::Slot;
use crate::paint::Paint;
use crate::picture::rect_of;
use crate::stroke::Stroke;
use crate::tool::Tool;

impl Canvas {
    pub(super) fn begin_stroke(
        &self,
        at: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Drag> {
        let slot = Slot::from_button(button)?;
        if let Some(refusal) = &self.brush.refusal {
            ctx.set_handled();
            ctx.call(&Handler::new(Paint::refuse), (refusal.clone(),));
            return None;
        }
        let keys = Keys {
            shift: modifiers.shift,
            alt: modifiers.alt,
        };
        let from = self.camera.picture_point(at);
        if self.tool == Tool::Gradient {
            return Some(Drag::Gradient {
                from,
                to: from,
                slot,
                keys,
            });
        }
        self.ink.begin(self.stroke(slot, from)?.keys(keys));
        Some(Drag::Draw { keys })
    }

    fn stroke(&self, slot: Slot, from: Point) -> Option<Stroke> {
        let brush = &self.brush;
        let ink = brush.colors.get(slot);
        let style = brush.filling.style(brush.colors, slot);
        let brushed = || {
            brush
                .brushing
                .map(|brushing| brushing.brush(brush.width, brush.antialiased))
        };
        Some(match self.tool {
            Tool::Paintbrush => Stroke::brush(brushed()?, Ink::Paint(ink), from),
            Tool::Eraser => Stroke::brush(
                brushed()?,
                Ink::Erase {
                    background: brush.colors.get(slot.other()),
                },
                from,
            ),
            Tool::FreeformShape => {
                Stroke::freeform(ink, brush.width, brush.antialiased, from).styled(style)
            }
            tool => match tool.shape() {
                Some(kind) => Stroke::shape(kind, ink, brush.width, from).styled(style),
                None => Stroke::new(tool.marks()?, ink, brush.width, from),
            },
        })
    }

    pub(super) fn keyed(
        &self,
        state: &mut CanvasState,
        event: &Event,
        key: Key,
        ctx: &mut EventCtx<'_>,
    ) {
        if let Some(Drag::Gradient { .. }) = state.drag {
            self.keyed_gradient(state, event, key, ctx);
            return;
        }
        let Some(Drag::Draw { keys }) = &mut state.drag else {
            return;
        };
        let down = matches!(event, Event::KeyPressed { .. });
        match key {
            Key::Escape if down => {
                state.drag = None;
                self.ink.take();
                ctx.release_pointer();
                ctx.set_handled();
            }
            Key::ShiftLeft | Key::ShiftRight => keys.shift = down,
            Key::AltLeft | Key::AltRight => keys.alt = down,
            _ => return,
        }
        if let Some(Drag::Draw { keys }) = state.drag {
            self.ink.change(|stroke| stroke.hold(keys));
        }
    }

    fn keyed_gradient(
        &self,
        state: &mut CanvasState,
        event: &Event,
        key: Key,
        ctx: &mut EventCtx<'_>,
    ) {
        let Some(Drag::Gradient {
            from,
            to,
            slot,
            keys,
        }) = &mut state.drag
        else {
            return;
        };
        let down = matches!(event, Event::KeyPressed { .. });
        match key {
            Key::Escape if down => {
                state.drag = None;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                ctx.run(Paint::cancel_gradient);
            }
            Key::ShiftLeft | Key::ShiftRight if keys.shift != down => {
                keys.shift = down;
                let end = constrain_angle(*from, *to, *keys);
                ctx.request_paint();
                ctx.call(&Handler::new(Paint::gradient), ((*from, end), *slot, false));
            }
            _ => {}
        }
    }

    pub(super) fn paint_stroke(
        &self,
        state: &mut CanvasState,
        ctx: &mut PaintCtx<'_>,
        whole: Rect,
        place: &impl Fn(Rect) -> Rect,
        corner: Point,
    ) {
        let bounds = ctx.bounds();
        let device = ctx.scale();
        let snap = |value: f32| (value * device).round() / device;
        let room = Point::new(snap(bounds.min_x()), snap(bounds.min_y()));
        let across = (bounds.width() * device).round().max(1.0);
        let down = (bounds.height() * device).round().max(1.0);
        let scale = self.camera.scale() * device;
        let erasing = match self.brush.transparent {
            true => Erasing::Checkerboard(checker::checkerboard(
                CHECKER_SQUARE,
                device,
                Point::new((corner.x - room.x) * device, (corner.y - room.y) * device),
            )),
            false => Erasing::Background,
        };
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "a rounded canvas extent is a small positive number of pixels"
        )]
        let placing = Placing {
            view: MarksView {
                width: across as u32,
                height: down as u32,
                scale,
            },
            offset: Point::new(
                (corner.x - room.x) * device / scale,
                (corner.y - room.y) * device / scale,
            ),
            erasing,
        };
        let revision = self.ink.revision();
        let mut floats = false;
        let overlay = self.ink.with_stroke(|stroke| {
            stroke.and_then(|stroke| {
                floats = stroke.floats();
                state.overlay.show(stroke, revision, placing)
            })
        });
        let source = match overlay {
            None => return,
            Some(Shown::Unchanged(source)) => source,
            Some(Shown::Redrawn(source)) => {
                ctx.request_frame();
                source
            }
        };
        let rect = Rect::new(room.x, room.y, across / device, down / device);
        let selection = self.selection.region();
        let clips = match selection.is_empty() || floats {
            true => vec![whole],
            false => state
                .overlay
                .bands(selection)
                .iter()
                .filter_map(|band| place(rect_of(*band)).intersection(&whole))
                .collect(),
        };
        for clip in clips {
            ctx.clipped(RoundedRect::new(clip, 0.0), |ctx| {
                ctx.draw_image(rect, source.clone());
            });
        }
    }
}
