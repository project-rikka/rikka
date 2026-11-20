use cut_project::TransitionKind;
use rikka_app::prelude::*;
use rikka_ui::{DRAG_SLOP, DragActions, DragIcon, DragStart, Offer};

use super::{MIME, TransitionPreviews, offered};
use crate::cut::Cut;
use crate::wording;

const WIDTH: f32 = 132.0;
const PICTURE: f32 = 74.0;
const NAME: f32 = 11.5;
const NAME_LINE: f32 = 18.0;
const NAME_GAP: f32 = 2.0;
const EDGE: f32 = 1.0;
const HOVERED: f32 = 2.0;
const PILL: Size = Size::new(150.0, 28.0);

#[derive(Debug)]
pub(super) struct Tile {
    pub(super) kind: TransitionKind,
    pub(super) transition_previews: TransitionPreviews,
    pub(super) ready: bool,
}

#[derive(Debug, Default)]
pub(super) struct PointerState {
    pressed: Option<Point>,
    hovering: bool,
}

impl Tile {
    fn picture_rect(bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), PICTURE)
    }

    fn enter(&self, state: &mut PointerState, ctx: &mut EventCtx<'_>) {
        state.hovering = true;
        self.transition_previews.play(Some(self.kind));
        let kind = self.kind;
        ctx.run(move |cut: &mut Cut| cut.start_transition_preview(Some(kind)));
        ctx.request_paint();
    }

    fn leave(&self, state: &mut PointerState, ctx: &mut EventCtx<'_>) {
        if !state.hovering {
            return;
        }
        state.hovering = false;
        self.transition_previews.play(None);
        ctx.run(|cut: &mut Cut| cut.start_transition_preview(None));
        ctx.request_paint();
    }

    fn lift(&self, state: &mut PointerState, ctx: &mut EventCtx<'_>, grip: Point) {
        state.pressed = None;
        ctx.release_pointer();
        self.leave(state, ctx);
        let name = wording::transition_kind(self.kind).to_string();
        let icon = DragIcon::new(
            PILL,
            Point::new(grip.x.min(PILL.width), PILL.height / 2.0),
            ctx.theme().clone(),
            move || {
                container(text(name.clone()).no_wrap())
                    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                    .background(palette().accent.scale_alpha(0.85))
                    .radius(Corner::Detail)
                    .size(PILL.width, PILL.height)
                    .build()
            },
        );
        let offer = Offer::new().with(MIME, offered(self.kind));
        ctx.start_drag(DragStart::new(offer, icon).actions(DragActions::COPY));
        ctx.set_handled();
    }
}

impl Widget for Tile {
    type State = PointerState;

    fn paint(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let hovered = ctx.hovered();
        let picture = Tile::picture_rect(bounds);
        ctx.draw_quad(
            Quad::new(picture)
                .fill(palette.surface_variant)
                .radius(Corner::Detail),
        );
        if let Some(source) = self.transition_previews.picture(self.kind) {
            ctx.clipped(picture, |ctx| ctx.draw_image(picture, source));
        }
        let (width, edge) = match hovered {
            true => (HOVERED, palette.accent),
            false => (EDGE, palette.outline),
        };
        ctx.draw_quad(
            Quad::new(picture)
                .stroke(width, edge)
                .radius(Corner::Detail),
        );
        let ink = match self.ready || hovered {
            true => palette.text,
            false => palette.text_muted,
        };
        let name = wording::transition_kind(self.kind).to_string();
        let line = Rect::new(
            bounds.x(),
            picture.max_y() + NAME_GAP,
            bounds.width(),
            NAME_LINE,
        );
        ctx.draw_text(line, TextRun::new(name, NAME).no_wrap().ellipsized(), ink);
        if hovered || state.hovering {
            ctx.request_frame();
        }
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        let bounds = ctx.bounds();
        match event {
            Event::PointerEntered => self.enter(state, ctx),
            Event::PointerExited => self.leave(state, ctx),
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.pressed = Some(Point::new(position.x - bounds.x(), position.y - bounds.y()));
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
            }
            Event::PointerMoved(position) => {
                let Some(from) = state.pressed else {
                    return;
                };
                let local = Point::new(position.x - bounds.x(), position.y - bounds.y());
                if (local.x - from.x).hypot(local.y - from.y) >= DRAG_SLOP {
                    self.lift(state, ctx, from);
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.pressed.take().is_some() => {
                ctx.release_pointer();
                ctx.set_handled();
                if self.ready {
                    let kind = self.kind;
                    self.transition_previews.play(None);
                    state.hovering = false;
                    ctx.run(move |cut: &mut Cut| cut.transit(kind));
                }
            }
            _ => {}
        }
    }

    fn cursor(&self, _state: &Self::State, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::Grab)
    }

    fn behavior(&self, _state: &PointerState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Tile) -> bool {
        self.kind == other.kind
            && self.ready == other.ready
            && self.transition_previews.same(&other.transition_previews)
    }
}

impl View for Tile {
    fn build(self) -> Element {
        Element::new(self).size(WIDTH, PICTURE + NAME_GAP + NAME_LINE)
    }
}
