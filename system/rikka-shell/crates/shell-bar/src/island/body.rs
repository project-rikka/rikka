use rikka_renderer::Joined;
use rikka_settings::session::appearance::TaskbarPosition;
use rikka_theme::Corner;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect, Size};
use rikka_ui::{
    Action, Align, Anchor, AnchorEdge, Behavior, Curve, DurationToken, Easing, Element, Event,
    EventCtx, IntoHandler, Layout, Length, PaintCtx, Quad, View, Widget, fade, motion,
};
use shell_kit::taskbar::{DEPTH, Edge};

use crate::axis;
use crate::dock::Dock;
use crate::island::cover_transition::{CoverTransition, CoverTransitionState};
use crate::island::face::Face;

const ROUNDING: f32 = Corner::Window.radius();

const HAIRLINE: f32 = 0.5;

pub(crate) const MARGIN: f32 = 8.0;

const SWELL: &str = "island";

const STAGE: &str = "island.stage";

const BERTH: &str = "island.berth";

const DEPARTURE: Curve = Curve::Tween {
    easing: Easing::StandardDecel,
    duration: DurationToken::Short2,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hang {
    Over,
    Below { centre: f32 },
}

pub(crate) struct Card {
    element: Element,
    width: f32,
    hang: Hang,
}

impl Card {
    #[must_use]
    pub(crate) fn over(element: Element, width: f32) -> Card {
        Card {
            element,
            width,
            hang: Hang::Over,
        }
    }

    #[must_use]
    pub(crate) fn below(element: Element, width: f32, centre: f32) -> Card {
        Card {
            element,
            width,
            hang: Hang::Below { centre },
        }
    }

    fn berth(&self, dock: Dock) -> (Rect, AnchorEdge) {
        let edge = dock.edge();
        let frame = Rect::from_origin_size(Point::ZERO, dock.surface());
        let length = edge.length(frame);
        let width = self.width;
        let local = match (edge.is_vertical(), self.hang) {
            (false, Hang::Over) => Rect::new((length - width) / 2.0, 0.0, width, 0.0),
            (false, Hang::Below { centre }) => {
                let most = (length - width - MARGIN).max(MARGIN);
                let start = (centre - width / 2.0).clamp(MARGIN, most);
                Rect::new(start, DEPTH, width, 0.0)
            }
            (true, Hang::Over) => Rect::new(0.0, 0.0, length, width),
            (true, Hang::Below { centre }) => Rect::new(centre, DEPTH, 0.0, width),
        };
        let toward = match edge.position() {
            TaskbarPosition::Top => AnchorEdge::BelowStart,
            TaskbarPosition::Bottom => AnchorEdge::AboveStart,
            TaskbarPosition::Left | TaskbarPosition::Right => AnchorEdge::Center,
        };
        (edge.place(frame, local), toward)
    }
}

pub(crate) struct Body {
    dock: Dock,
    face: Element,
    card: Option<Card>,
    cover_transition: CoverTransition,
    on_hover: Action<(bool,)>,
}

pub(crate) fn body<Marker>(
    dock: Dock,
    face: Face,
    card: Option<Card>,
    cover_transition: CoverTransition,
    on_hover: impl IntoHandler<(bool,), (), Marker>,
) -> Body {
    Body {
        dock,
        face: face.build(),
        card,
        cover_transition,
        on_hover: Action::new(on_hover),
    }
}

impl Body {
    fn edge(&self) -> Edge {
        self.dock.edge()
    }
}

impl Widget for Body {
    type State = CoverTransitionState;

    fn layout(&self) -> Layout {
        axis::layout(self.edge().axis().across())
            .align_items(Align::Center)
            .justify(self.edge().justify())
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, flown: &mut CoverTransitionState, ctx: &mut PaintCtx<'_>) {
        let edge = self.edge();
        let origin = ctx.bounds().origin;
        let frame = Rect::from_origin_size(Point::ZERO, ctx.bounds().size);
        let Some(face) = ctx.child_rect(0).map(|rect| rect.translate(-origin)) else {
            return;
        };
        let card = self
            .card
            .as_ref()
            .and_then(|_| newest(ctx))
            .map(|rect| rect.translate(-origin));
        let near = edge.local(frame, face);
        let (target, curve) = match card.map(|card| edge.local(frame, card)) {
            Some(card) => (
                Rect::new(card.x(), DEPTH, card.width(), card.max_y() - DEPTH),
                motion::EMERGE,
            ),
            None => (
                Rect::new(near.x(), DEPTH, near.width(), 0.0),
                motion::RESHAPE,
            ),
        };
        let swell = ctx.animate_curve(SWELL, target, curve);
        let swell = Rect::new(
            swell.x(),
            DEPTH,
            swell.width().max(0.0),
            swell.height().max(0.0),
        );

        let rounding = ROUNDING.min(swell.height() / 2.0).min(swell.width() / 2.0);
        let radii = edge.radii(CornerRadii::new(0.0, 0.0, rounding, rounding));
        if swell.height() >= HAIRLINE {
            let fill = ctx.palette().background;
            let limb = RoundedRect::new(edge.place(frame, swell).translate(origin), radii);
            ctx.draw_quad(Quad::new(limb.rect).fill(fill).radius(radii));
            ctx.glass(limb);
            let strip = edge
                .place(frame, Rect::new(0.0, 0.0, edge.length(frame), DEPTH))
                .translate(origin);
            let joined = Joined::new(RoundedRect::rect(strip), limb);
            for (square, hollow) in joined.iter().flat_map(Joined::fillets).flatten() {
                ctx.draw_quad(Quad::new(square).fill(fill).cut_out(hollow));
            }
        }

        self.cover_transition.paint(flown, ctx, face, card, curve);

        let start = swell.min_x().min(near.min_x());
        let end = swell.max_x().max(near.max_x());
        let reach = Rect::new(start, 0.0, end - start, DEPTH + swell.height());
        ctx.set_children_clip(RoundedRect::new(
            edge.place(frame, reach).translate(origin),
            radii,
        ));
    }

    fn event(&self, _state: &mut CoverTransitionState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerEntered => ctx.act(&self.on_hover, (true,)),
            Event::PointerExited => ctx.act(&self.on_hover, (false,)),
            _ => {}
        }
    }

    fn behavior(&self, _state: &CoverTransitionState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn debug_name(&self) -> &'static str {
        "IslandBody"
    }
}

fn newest(ctx: &PaintCtx<'_>) -> Option<Rect> {
    (2..).map_while(|index| ctx.child_rect(index)).last()
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("open", &self.card.is_some())
            .finish_non_exhaustive()
    }
}

impl View for Body {
    fn build(mut self) -> Element {
        let face = std::mem::replace(&mut self.face, Element::empty());
        let (spot, card) = match self.card.as_mut() {
            Some(card) => {
                let (spot, toward) = card.berth(self.dock);
                let element = std::mem::replace(&mut card.element, Element::empty())
                    .anchored(Anchor::new(BERTH, toward))
                    .transition(fade().exit(DEPARTURE))
                    .keyed(STAGE);
                (spot, Some(element))
            }
            None => (Rect::from_origin_size(Point::ZERO, Size::ZERO), None),
        };
        let berth = Element::new(Berth { spot }).keyed(BERTH);
        let children = [face, berth].into_iter().chain(card).collect();
        Element::with_children(self, children)
    }
}

struct Berth {
    spot: Rect,
}

impl Widget for Berth {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(self.spot.width()),
                Length::px(self.spot.height()),
            )
            .placed_at(self.spot.origin)
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn debug_name(&self) -> &'static str {
        "IslandBerth"
    }
}
