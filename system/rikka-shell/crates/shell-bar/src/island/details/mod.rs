mod idle;
mod media;
mod opened;
mod peek;
mod preview;
mod rows;

use rikka_types::Point;
use rikka_ui::{
    Curve, DurationToken, Easing, Element, Inset, Length, Styled, View, fade, offset, scale,
};
use rikka_widgets::container;

use crate::island::body::Card;
use crate::island::cover_transition::CoverTarget;
use crate::island::live::Live;
use crate::island::state::Island;

const OPEN: f32 = 360.0;
const PEEK: f32 = 320.0;
const SIDE: f32 = 14.0;
const TOP: f32 = 18.0;
const BOTTOM: f32 = 12.0;
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
    let (showing, width, (content, cover_target)) = match island.peek() {
        Some(activity_event) => (Showing::Peek, PEEK, peek::view(activity_event, island)),
        None if island.is_open() => (Showing::Open, OPEN, opened::view(island, live_activities)),
        None => {
            let aim = island.previewing()?;
            let (content, width) = preview::view(island, aim)?;
            let content = arriving(content, Showing::Preview(aim.space, aim.key.clone()));
            let left = preview::left(aim.centre, width, island.width());
            return Some(Details {
                card: Card::below(content, width, left),
                cover_target: None,
            });
        }
    };
    let content = arriving(content, showing);
    let card = container(content)
        .padding(Inset::exact(TOP, SIDE, BOTTOM, SIDE))
        .width(Length::px(width))
        .build();
    Some(Details {
        card: Card::over(card, width),
        cover_target: cover_target.map(|cover_target| cover_target.inset(Point::new(SIDE, TOP))),
    })
}

fn arriving(content: Element, showing: Showing) -> Element {
    content
        .transition(
            (fade() + scale(GROWN_FROM) + offset(0.0, -LIFT))
                .enter(ARRIVAL)
                .delay(DurationToken::Short1),
        )
        .keyed((SHOWING, showing))
}
