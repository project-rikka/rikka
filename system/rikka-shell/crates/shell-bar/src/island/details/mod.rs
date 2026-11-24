mod idle;
mod media;
mod opened;
mod peek;
mod preview;
mod rows;

use rikka_types::Point;
use rikka_ui::{Curve, DurationToken, Easing, Element, Length, Styled, View, fade, scale};
use rikka_widgets::container;
use shell_kit::taskbar::Edge;

use crate::island::body::Card;
use crate::island::cover_transition::CoverTarget;
use crate::island::live::Live;
use crate::island::state::Island;
use crate::rim;

const OPEN: f32 = 360.0;
const PEEK: f32 = 320.0;
const SIDE: f32 = 14.0;
const NEAR: f32 = 18.0;
const FAR: f32 = 12.0;
const LIFT: f32 = 6.0;
const GROWN_FROM: f32 = 0.96;

const SHOWING: &str = "island.showing";

const ARRIVAL: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Medium1,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Showing {
    Peek,
    Open,
    Preview(usize, String),
}

pub(super) struct Details {
    pub(crate) card: Card,
    pub(crate) cover_target: Option<CoverTarget>,
}

pub(crate) fn details(island: &Island, live_activities: &[Live]) -> Option<Details> {
    let dock = island.dock();
    let edge = dock.edge();
    let (showing, width, (content, cover_target)) = match island.peek() {
        Some(activity_event) => (Showing::Peek, PEEK, peek::view(activity_event, island)),
        None if island.is_open() => (Showing::Open, OPEN, opened::view(island, live_activities)),
        None => {
            let aim = island.previewing()?;
            let (content, width) = preview::view(island, aim)?;
            let content = arriving(content, edge, Showing::Preview(aim.space, aim.key.clone()));
            let centre = dock.axis().coord(aim.bounds.center());
            return Some(Details {
                card: Card::below(content, width, centre),
                cover_target: None,
            });
        }
    };
    let content = arriving(content, edge, showing);
    let padding = edge.inset(NEAR, FAR, SIDE, SIDE);
    let card = container(content)
        .padding(padding)
        .width(Length::px(width))
        .build();
    Some(Details {
        card: Card::over(card, width),
        cover_target: cover_target
            .map(|cover_target| cover_target.inset(Point::new(padding.left(), padding.top()))),
    })
}

fn arriving(content: Element, edge: Edge, showing: Showing) -> Element {
    content
        .transition(
            (fade() + scale(GROWN_FROM) + rim::shifted(edge, LIFT))
                .enter(ARRIVAL)
                .delay(DurationToken::Short1),
        )
        .keyed((SHOWING, showing))
}
