use std::sync::Arc;

use paint_document::FigureKind;
use rikka_app::prelude::*;
use rikka_figure::{Lettering, Typeset};
use rikka_figure_editing::{Typing, TypingOutcome, paint_typing};
use rikka_ui::ImageSource;

use super::{Canvas, CanvasState, Drag};
use crate::paint::Paint;
use crate::typing::Draft;

const HANDLE: f32 = 9.0;
const HANDLE_GAP: f32 = 6.0;
const REACH: f32 = 6.0;
const FRAME: f32 = 1.0;

#[derive(Debug, Default)]
pub(crate) struct TextOverlay {
    cache: Option<TypesetCache>,
    caret: Option<Rect>,
}

#[derive(Debug)]
struct TypesetCache {
    lettering: Lettering,
    extent: (u32, u32),
    typeset: Typeset,
    ink: Option<(Rect, ImageSource)>,
}

pub(super) struct Placing<'a> {
    pub(super) whole: Rect,
    pub(super) pixelated: bool,
    pub(super) place: &'a dyn Fn(Rect) -> Rect,
    pub(super) at: &'a dyn Fn(f32, f32) -> Point,
}

impl TextOverlay {
    pub(super) fn caret(&self) -> Option<Rect> {
        self.caret
    }

    fn typeset(&mut self, lettering: Lettering, extent: (u32, u32)) -> &TypesetCache {
        let stale = self
            .cache
            .as_ref()
            .is_none_or(|typeset| typeset.lettering != lettering || typeset.extent != extent);
        if stale {
            let typeset = Typeset::new(&lettering);
            let ink = typeset
                .raster(Rect::new(0.0, 0.0, extent.0 as f32, extent.1 as f32))
                .map(|raster| {
                    let area = Rect::new(
                        raster.left as f32,
                        raster.top as f32,
                        raster.width as f32,
                        raster.height as f32,
                    );
                    let source = ImageSource::Rgba {
                        width: raster.width,
                        height: raster.height,
                        pixels: Arc::from(raster.pixels),
                    };
                    (area, source)
                });
            self.cache = Some(TypesetCache {
                lettering,
                extent,
                typeset,
                ink,
            });
        }
        self.cache
            .as_ref()
            .unwrap_or_else(|| unreachable!("the text was just set"))
    }
}

impl Canvas {
    fn extent(&self) -> (u32, u32) {
        let extent = self.camera.extent();
        (extent.width as u32, extent.height as u32)
    }

    fn typeset_of(&self, draft: &Draft) -> Typeset {
        self.typing
            .typeset(draft.lettering(&self.letters, self.brush.colors.primary))
    }

    fn frame_of(&self, typeset: &Typeset) -> Rect {
        let layout = typeset.layout();
        let least = typeset.line_height() / 2.0;
        Rect::new(
            layout.x(),
            layout.y(),
            layout.width().max(least),
            layout.height(),
        )
    }

    fn handle_at(&self, frame: Rect) -> Point {
        let scale = self.camera.scale();
        let offset = (HANDLE / 2.0 + HANDLE_GAP) / scale;
        Point::new(frame.min_x() - offset, frame.min_y() - offset)
    }

    pub(super) fn grips_handle(&self, at: Point) -> bool {
        let Some(draft) = self.typing.draft() else {
            return false;
        };
        let handle = self.handle_at(self.frame_of(&self.typeset_of(&draft)));
        let point = self.camera.picture_point(at);
        let reach = (HANDLE / 2.0 + 2.0) / self.camera.scale();
        (point.x - handle.x).abs() <= reach && (point.y - handle.y).abs() <= reach
    }

    pub(super) fn press_text(
        &self,
        at: Point,
        button: PointerButton,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Drag> {
        if button != PointerButton::Primary {
            return None;
        }
        let point = self.camera.picture_point(at);
        let Some(draft) = self.typing.draft() else {
            if let Some((_, figure)) = self.figure_under(point)
                && figure.kind() == FigureKind::Text
                && self.figures.editable
            {
                ctx.call(&Handler::new(Paint::edit_text_figure), (figure,));
                return Some(Drag::Caret);
            }
            if let Some(refusal) = &self.brush.refusal {
                ctx.set_handled();
                ctx.call(&Handler::new(Paint::refuse), (refusal.clone(),));
                return None;
            }
            self.typing
                .begin((point.x.floor() as i32, point.y.floor() as i32));
            return Some(Drag::Caret);
        };
        if self.grips_handle(at) {
            return Some(Drag::Carry {
                grabbed: point,
                from: draft.origin,
            });
        }
        draft.document.edit(|content| content.clear_preedit());
        let typeset = self.typeset_of(&draft);
        let reach = REACH / self.camera.scale();
        if !self.frame_of(&typeset).inflate(reach).contains(point) {
            ctx.set_handled();
            ctx.run(Paint::commit_text);
            return None;
        }
        draft.document.interrupt();
        let hit = typeset.hit(point);
        draft
            .document
            .place_cursor(hit.at, hit.affinity, modifiers.shift);
        Some(Drag::Caret)
    }

    pub(super) fn drag_text(&self, drag: Drag, at: Point) {
        let Some(draft) = self.typing.draft() else {
            return;
        };
        let point = self.camera.picture_point(at);
        match drag {
            Drag::Caret => {
                let hit = self.typeset_of(&draft).hit(point);
                draft.document.place_cursor(hit.at, hit.affinity, true);
            }
            Drag::Carry { grabbed, from } => self.typing.place((
                from.0 + (point.x - grabbed.x).round() as i32,
                from.1 + (point.y - grabbed.y).round() as i32,
            )),
            _ => {}
        }
    }

    pub(super) fn type_event(
        &self,
        state: &mut CanvasState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(draft) = self.typing.draft() else {
            return false;
        };
        let caret = state.lettered.caret();
        let typed = Typing::new(&draft.document, &mut state.menu)
            .caret(caret)
            .handle(event, || self.typeset_of(&draft), ctx);
        match typed {
            TypingOutcome::Ignored => return false,
            TypingOutcome::Dropped => {
                ctx.run(Paint::drop_text);
            }
            TypingOutcome::Committed => {
                ctx.run(Paint::commit_text);
            }
            TypingOutcome::Edited | TypingOutcome::Unchanged => {}
        }
        ctx.request_paint();
        true
    }

    pub(super) fn summon_text_menu(
        &self,
        state: &mut CanvasState,
        at: Point,
        position: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let Some(draft) = self.typing.draft() else {
            return false;
        };
        let point = self.camera.picture_point(at);
        let typeset = self.typeset_of(&draft);
        let reach = REACH / self.camera.scale();
        if !self.frame_of(&typeset).inflate(reach).contains(point) {
            return false;
        }
        let document = &draft.document;
        document.edit(|content| content.clear_preedit());
        let hit = typeset.hit(point);
        document.place_cursor_unless_selected(hit.at, hit.affinity);
        state.menu.open(text_menu(document), position, ctx);
        true
    }

    pub(super) fn paint_text(
        &self,
        state: &mut CanvasState,
        ctx: &mut PaintCtx<'_>,
        placing: &Placing<'_>,
    ) {
        let draft = self.typing.draft();
        let pending_lettering = match &draft {
            Some(draft) => Some(draft.display_lettering(&self.letters, self.brush.colors.primary)),
            None => self.typing.settling(),
        };
        let Some(lettering) = pending_lettering else {
            state.lettered.caret = None;
            return;
        };
        let extent = self.extent();
        let bounds = ctx.bounds();
        let accent = palette().accent;
        let laid_out = state.lettered.typeset(lettering, extent);
        if let Some((area, source)) = &laid_out.ink {
            let rect = (placing.place)(*area);
            ctx.clipped(RoundedRect::new(placing.whole, 0.0), |ctx| {
                match placing.pixelated {
                    true => ctx.draw_image_pixelated(rect, source.clone()),
                    false => ctx.draw_image(rect, source.clone()),
                }
            });
        }
        let typeset = laid_out.typeset.clone();
        let Some(draft) = draft else {
            state.lettered.caret = None;
            return;
        };
        let frame = self.frame_of(&typeset);
        let framed = (placing.place)(frame);
        ctx.draw_quad(Quad::new(framed).stroke(FRAME, accent));
        let handle = self.handle_at(frame);
        let handle = (placing.at)(handle.x, handle.y);
        ctx.draw_quad(
            Quad::new(Rect::from_center(handle, Size::new(HANDLE, HANDLE)))
                .fill(palette().surface)
                .stroke(FRAME, accent),
        );
        let at = |point: Point| Some((placing.at)(point.x, point.y));
        state.lettered.caret = paint_typing(ctx, &typeset, &draft.document, &at)
            .map(|caret| caret.translate(Point::new(-bounds.x(), -bounds.y())));
    }
}
