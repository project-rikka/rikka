use paint_document::{Area, Combine, Shape};
use rikka_app::prelude::*;

use super::path::Path;
use super::{Canvas, CanvasState, Drag, marquee};
use crate::colors::Slot;
use crate::paint::Paint;
use crate::tool::Tool;

const LASSO_SPACING: f32 = 1.0;

impl Canvas {
    pub(super) fn begin_selecting(
        &self,
        state: &mut CanvasState,
        at: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Drag> {
        if !self.selectable {
            return None;
        }
        let from = self.camera.picture_point(at);
        let plain = button == PointerButton::Primary
            && !(modifiers.control || modifiers.alt || modifiers.shift)
            && self.selecting == Combine::Replace;
        if plain && self.tool != Tool::PolygonLasso && self.selection.holds(from.x, from.y) {
            return Some(Drag::Move {
                from,
                base: self.selection.offset(),
                by: (0, 0),
            });
        }
        let combine = marquee::combine_for(button, modifiers, self.selecting)?;
        match self.tool {
            Tool::RectangleSelect | Tool::EllipseSelect => Some(Drag::Select {
                from,
                combine,
                square: modifiers.shift,
                oval: self.tool == Tool::EllipseSelect,
                marked: Area::default(),
            }),
            Tool::Lasso => {
                state.path = Some(Path::lasso(combine, from));
                ctx.request_paint();
                Some(Drag::Lasso)
            }
            Tool::MagicWand => {
                ctx.set_handled();
                ctx.request_focus();
                ctx.call(
                    &Handler::new(Paint::magic_wand),
                    ((from.x, from.y), combine),
                );
                None
            }
            _ => None,
        }
    }

    pub(super) fn polygon_pressed(
        &self,
        state: &mut CanvasState,
        at: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if self.tool != Tool::PolygonLasso {
            state.path = None;
            return false;
        }
        let panning = button == PointerButton::Middle || self.grip.space_held();
        if panning || !self.selectable {
            return false;
        }
        let point = self.camera.picture_point(at);
        match &mut state.path {
            Some(path) if path.is_polygon() => {
                if path.closes(point, ctx.presses(), self.camera.scale()) {
                    let path = state.path.take();
                    self.finish(path, ctx);
                } else {
                    path.add(point);
                }
            }
            _ => {
                let Some(combine) = marquee::combine_for(button, modifiers, self.selecting) else {
                    return false;
                };
                state.path = Some(Path::polygon(combine, point));
            }
        }
        ctx.request_focus();
        ctx.request_paint();
        ctx.set_handled();
        true
    }

    pub(super) fn polygon_moved(&self, state: &mut CanvasState, at: Point, ctx: &mut EventCtx<'_>) {
        if let Some(path) = &mut state.path
            && path.is_polygon()
            && path.aim(self.camera.picture_point(at))
        {
            ctx.request_paint();
        }
    }

    pub(super) fn path_keyed(
        &self,
        state: &mut CanvasState,
        event: &Event,
        key: Key,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(path) = &mut state.path else {
            return false;
        };
        if !matches!(event, Event::KeyPressed { .. }) {
            return false;
        }
        if !path.is_polygon() {
            if key != Key::Escape {
                return false;
            }
            state.path = None;
            state.drag = None;
            ctx.release_pointer();
            ctx.request_paint();
            ctx.set_handled();
            return true;
        }
        match key {
            Key::Enter => {
                let path = state.path.take();
                self.finish(path, ctx);
            }
            Key::Escape => state.path = None,
            Key::Backspace | Key::Delete => {
                if !path.retract() {
                    state.path = None;
                }
            }
            _ => return false,
        }
        ctx.request_paint();
        ctx.set_handled();
        true
    }

    pub(super) fn select_dragged(
        &self,
        state: &mut CanvasState,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        let point = self.camera.picture_point(at);
        match &mut state.drag {
            Some(Drag::Select {
                from,
                combine,
                square,
                oval,
                marked,
            }) => {
                let area = self.marked(*from, at, *square);
                if area != *marked {
                    *marked = area;
                    let shape = self.marquee(area, *oval).anti_aliased(false);
                    self.selection
                        .preview(*combine, shape.region(self.canvas_area()));
                }
            }
            Some(Drag::Lasso) => {
                if let Some(path) = &mut state.path {
                    path.extend(point, LASSO_SPACING / self.camera.scale());
                    ctx.request_paint();
                }
            }
            Some(Drag::Move { from, base, by }) => {
                let moved = (
                    (point.x - from.x).round() as i32,
                    (point.y - from.y).round() as i32,
                );
                if moved != *by {
                    *by = moved;
                    self.selection
                        .displace((base.0 + moved.0, base.1 + moved.1));
                }
            }
            _ => {}
        }
    }

    pub(super) fn select_released(
        &self,
        state: &mut CanvasState,
        drag: Drag,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) {
        match drag {
            Drag::Select {
                from,
                combine,
                square,
                oval,
                ..
            } => {
                let area = self.marked(from, at, square);
                match (area.is_empty(), combine) {
                    (true, Combine::Replace) => self.deselect(ctx),
                    (true, _) => self.selection.withdraw(),
                    (false, _) => self.commit_shape(self.marquee(area, oval), combine, ctx),
                }
            }
            Drag::Lasso => {
                if let Some(path) = &mut state.path {
                    path.extend(
                        self.camera.picture_point(at),
                        LASSO_SPACING / self.camera.scale(),
                    );
                }
                let path = state.path.take();
                self.finish(path, ctx);
            }
            Drag::Move { from, base, by } => {
                self.selection.displace(base);
                match (by, self.tool) {
                    ((0, 0), Tool::MagicWand) => {
                        ctx.call(
                            &Handler::new(Paint::magic_wand),
                            ((from.x, from.y), Combine::Replace),
                        );
                    }
                    ((0, 0), _) => self.deselect(ctx),
                    _ => {
                        ctx.call(&Handler::new(Paint::move_selection), (by,));
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn paint_path(
        &self,
        state: &CanvasState,
        ctx: &mut PaintCtx<'_>,
        place: &impl Fn(f32, f32) -> Point,
    ) {
        if let Some(path) = &state.path
            && matches!(self.tool, Tool::Lasso | Tool::PolygonLasso)
        {
            path.draw(ctx, place);
        }
    }

    pub(super) fn pour(&self, at: Point, button: PointerButton, ctx: &mut EventCtx<'_>) {
        let Some(slot) = Slot::from_button(button) else {
            return;
        };
        ctx.set_handled();
        if let Some(refusal) = &self.brush.refusal {
            ctx.call(&Handler::new(Paint::refuse), (refusal.clone(),));
            return;
        }
        if self.pixel(at).is_some() {
            let point = self.camera.picture_point(at);
            ctx.call(&Handler::new(Paint::pour), ((point.x, point.y), slot));
        }
    }

    fn finish(&self, path: Option<Path>, ctx: &mut EventCtx<'_>) {
        let Some(path) = path else {
            return;
        };
        match path.shape(self.smoothing) {
            Some(shape) => self.commit_shape(shape, path.combine, ctx),
            None if path.combine == Combine::Replace => self.deselect(ctx),
            None => {}
        }
    }

    fn commit_shape(&self, shape: Shape, combine: Combine, ctx: &mut EventCtx<'_>) {
        self.selection
            .preview(combine, shape.region(self.canvas_area()));
        self.selection.commit();
        ctx.call(&Handler::new(Paint::select_shape), (shape, combine));
    }

    fn deselect(&self, ctx: &mut EventCtx<'_>) {
        ctx.run(Paint::deselect);
    }

    fn marquee(&self, area: Area, oval: bool) -> Shape {
        let (left, top) = (area.left as f32, area.top as f32);
        let (width, height) = (area.width as f32, area.height as f32);
        match oval {
            true => Shape::ellipse(left, top, width, height).anti_aliased(self.smoothing),
            false => Shape::rectangle(left, top, width, height),
        }
    }

    fn canvas_area(&self) -> Area {
        let extent = self.camera.extent();
        Area::sized(extent.width as u32, extent.height as u32)
    }
}
