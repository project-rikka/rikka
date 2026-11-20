use rikka_renderer::Joined;
use rikka_theme::Corner;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect};
use rikka_ui::{
    Action, Align, Behavior, Curve, DurationToken, Easing, Element, Event, EventCtx, IntoHandler,
    Layout, Length, PaintCtx, Quad, View, Widget, fade, motion,
};
use shell_kit::taskbar::Shape;

use crate::island::cover_transition::{CoverTransition, CoverTransitionState};
use crate::island::face::Face;
use crate::island::stage::{Hang, stage};

const ROUNDING: f32 = Corner::Window.radius();

const HAIRLINE: f32 = 0.5;

const SWELL: &str = "island";

const STAGE: &str = "island.stage";

const DEPARTURE: Curve = Curve::Tween {
    easing: Easing::StandardDecel,
    duration: DurationToken::Short2,
};

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
    pub(crate) fn below(element: Element, width: f32, left: f32) -> Card {
        Card {
            element,
            width,
            hang: Hang::Below { left },
        }
    }

    fn placed(&self, stage: Rect) -> Rect {
        match self.hang {
            Hang::Over => Rect::new(
                stage.center().x - self.width / 2.0,
                stage.y(),
                self.width,
                stage.height(),
            ),
            Hang::Below { left } => {
                let depth = Shape.reserved();
                Rect::new(
                    stage.x() + left,
                    stage.y() + depth,
                    self.width,
                    (stage.height() - depth).max(0.0),
                )
            }
        }
    }
}

pub(crate) struct Body {
    face: Element,
    card: Option<Card>,
    cover_transition: CoverTransition,
    on_hover: Action<(bool,)>,
}

pub(crate) fn body<Marker>(
    face: Face,
    card: Option<Card>,
    cover_transition: CoverTransition,
    on_hover: impl IntoHandler<(bool,), (), Marker>,
) -> Body {
    Body {
        face: face.build(),
        card,
        cover_transition,
        on_hover: Action::new(on_hover),
    }
}

impl Widget for Body {
    type State = CoverTransitionState;

    fn layout(&self) -> Layout {
        Layout::column()
            .align_items(Align::Center)
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, flown: &mut CoverTransitionState, ctx: &mut PaintCtx<'_>) {
        let origin = ctx.bounds().origin;
        let Some(face) = ctx.child_rect(0).map(|rect| rect.translate(-origin)) else {
            return;
        };
        let card = self
            .card
            .as_ref()
            .and_then(|card| newest(ctx).map(|stage| card.placed(stage)))
            .map(|rect| rect.translate(-origin));
        let depth = Shape.reserved();
        let (target, curve) = match card {
            Some(card) => (
                Rect::new(card.x(), depth, card.width(), card.max_y() - depth),
                motion::EMERGE,
            ),
            None => (
                Rect::new(face.x(), depth, face.width(), 0.0),
                motion::RESHAPE,
            ),
        };
        let swell = ctx.animate_curve(SWELL, target, curve);
        let swell = Rect::new(
            swell.x(),
            depth,
            swell.width().max(0.0),
            swell.height().max(0.0),
        )
        .translate(origin);

        let rounding = ROUNDING.min(swell.height() / 2.0).min(swell.width() / 2.0);
        let radii = CornerRadii::new(0.0, 0.0, rounding, rounding);
        if swell.height() >= HAIRLINE {
            let fill = ctx.palette().background;
            let limb = RoundedRect::new(swell, radii);
            ctx.draw_quad(Quad::new(swell).fill(fill).radius(radii));
            ctx.glass(limb);
            let strip = Rect::new(origin.x, origin.y, ctx.bounds().width(), depth);
            let joined = Joined::new(RoundedRect::rect(strip), limb);
            for (square, hollow) in joined.iter().flat_map(Joined::fillets).flatten() {
                ctx.draw_quad(Quad::new(square).fill(fill).cut_out(hollow));
            }
        }

        self.cover_transition.paint(flown, ctx, face, card, curve);

        let face = face.translate(origin);
        let left = swell.min_x().min(face.min_x());
        let right = swell.max_x().max(face.max_x());
        ctx.set_children_clip(RoundedRect::new(
            Rect::new(left, origin.y, right - left, depth + swell.height()),
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
    (1..).map_while(|index| ctx.child_rect(index)).last()
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
        let card = self.card.as_mut().map(|card| {
            let element = std::mem::replace(&mut card.element, Element::empty());
            stage(element, card.hang)
                .build()
                .transition(fade().exit(DEPARTURE))
                .keyed(STAGE)
        });
        let children = std::iter::once(face).chain(card).collect();
        Element::with_children(self, children)
    }
}
