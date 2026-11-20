use std::sync::Arc;

use rikka_theme::Corner;
use rikka_types::{Point, PointerButton, Rect, RoundedRect, WindowView};
use rikka_ui::{
    Action, DRAG_SLOP, Element, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx, Quad,
    TextRole, TextRun, View, Widget, motion, palette, theme,
};
use switcher_layout::Arrangement;

use crate::board::Face;

const BORDER: f32 = 1.0;
const RING: f32 = 2.0;
const SELECTED_FILL: f32 = 0.16;
const ICON: f32 = 16.0;
const ICON_GAP: f32 = 8.0;

pub(crate) struct Panel {
    laid: Arc<Arrangement>,
    faces: Arc<[Face]>,
    selected: usize,
    on_point: Action<(usize,)>,
    on_choose: Action<(usize,)>,
    on_dismiss: Action,
    leaving: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PanelState {
    scroll: f32,
    scrolled: f32,
    rest: Option<Point>,
    roused: bool,
    pressed: Option<Option<usize>>,
}

#[must_use]
pub(crate) fn panel(laid: Arc<Arrangement>, faces: Arc<[Face]>, selected: usize) -> Panel {
    Panel {
        laid,
        faces,
        selected,
        on_point: Action::default(),
        on_choose: Action::default(),
        on_dismiss: Action::default(),
        leaving: false,
    }
}

impl Panel {
    #[must_use]
    pub(super) fn on_point<Marker>(
        mut self,
        on_point: impl IntoHandler<(usize,), (), Marker>,
    ) -> Self {
        self.on_point = Action::new(on_point);
        self
    }

    #[must_use]
    pub(super) fn on_choose<Marker>(
        mut self,
        on_choose: impl IntoHandler<(usize,), (), Marker>,
    ) -> Self {
        self.on_choose = Action::new(on_choose);
        self
    }

    #[must_use]
    pub(crate) fn on_dismiss<Marker>(
        mut self,
        on_dismiss: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.on_dismiss = Action::new(on_dismiss);
        self
    }

    #[must_use]
    pub(crate) fn leaving(mut self, leaving: bool) -> Self {
        self.leaving = leaving;
        self
    }

    fn draw_cards(&self, lift: Point, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let theme = theme();
        let size = theme.typography().body_size * TextRole::Caption.scale();
        if let Some(card) = self.laid.cards.get(self.selected) {
            let ring = ctx
                .animate_curve("ring", card.frame, motion::INDICATOR)
                .translate(lift);
            ctx.draw_quad(
                Quad::new(ring)
                    .fill(palette.accent.with_alpha(SELECTED_FILL))
                    .stroke(RING, palette.accent)
                    .radius(Corner::Panel.radius()),
            );
        }
        for (card, face) in self.laid.cards.iter().zip(self.faces.iter()) {
            let header = card.header.translate(lift);
            let icon = Rect::new(header.min_x(), header.center().y - ICON / 2.0, ICON, ICON);
            ctx.draw_icon_source(icon, &face.icon, ICON, palette.text);
            let title = TextRun::new(Arc::clone(&face.title), size).no_wrap();
            let from = icon.max_x() + ICON_GAP;
            ctx.draw_text_line(header, from, header.max_x() - from, title, palette.text);
            let window = card.window.translate(lift);
            ctx.draw_window(
                WindowView::new(Arc::clone(&face.id), window)
                    .clip(RoundedRect::new(window, Corner::Item.radius())),
            );
        }
    }
}

impl Widget for Panel {
    type State = PanelState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut PanelState, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let origin = ctx.bounds().origin;
        let shape = RoundedRect::new(self.laid.panel.translate(origin), Corner::Window.radius());
        let shown = ctx.progress("shown", !self.leaving, motion::POP, motion::POP_OUT);
        state.scroll = self.laid.scrolled_to(self.selected, state.scroll);
        state.scrolled = ctx.animate_curve("scroll", state.scroll, motion::SCROLL);
        let lift = Point::new(origin.x, origin.y - state.scrolled);
        let viewport = self.laid.viewport.translate(origin);
        ctx.popped(shown, shape.rect.center(), |ctx| {
            ctx.draw_quad(
                Quad::new(shape.rect)
                    .fill(palette.surface)
                    .stroke(BORDER, palette.outline)
                    .radius(shape.radii),
            );
            ctx.glass(shape);
            ctx.clipped(RoundedRect::rect(viewport), |ctx| {
                self.draw_cards(lift, ctx);
            });
        });
    }

    fn event(&self, state: &mut PanelState, event: &Event, ctx: &mut EventCtx<'_>) {
        if self.leaving {
            return;
        }
        let origin = ctx.bounds().origin;
        let scroll = state.scrolled;
        let local = |position: Point| Point::new(position.x - origin.x, position.y - origin.y);
        let spot = |position: Point| self.laid.at(local(position), scroll);
        match event {
            Event::PointerMoved(position) => {
                let rest = *state.rest.get_or_insert(*position);
                state.roused |= rest.distance(*position) >= DRAG_SLOP;
                if state.roused
                    && let Some(index) = spot(*position)
                    && index != self.selected
                {
                    ctx.act(&self.on_point, (index,));
                }
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.set_handled();
                state.pressed = Some(spot(*position));
            }
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.set_handled();
                let Some(pressed) = state.pressed.take() else {
                    return;
                };
                let released = spot(*position);
                match released.filter(|_| pressed == released) {
                    Some(index) => ctx.act(&self.on_choose, (index,)),
                    None if pressed.is_none() && !self.laid.panel.contains(local(*position)) => {
                        ctx.act(&self.on_dismiss, ());
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.laid, &other.laid)
            && Arc::ptr_eq(&self.faces, &other.faces)
            && self.selected == other.selected
            && self.on_point == other.on_point
            && self.on_choose == other.on_choose
            && self.on_dismiss == other.on_dismiss
            && self.leaving == other.leaving
    }

    fn debug_name(&self) -> &'static str {
        "SwitcherPanel"
    }
}

impl std::fmt::Debug for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panel")
            .field("cards", &self.laid.cards.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl View for Panel {
    fn build(self) -> Element {
        Element::new(self)
    }
}
