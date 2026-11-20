mod expression;
mod eyes;
mod face;
mod hand;
mod holds;
mod mouth;
mod pad;

use animate_body::Expression;
use animate_hold::Holder;
use animate_motion::PoseSample;
use rikka_app::prelude::*;
use rikka_ui::{Anchor, AnchorEdge};
use rikka_widgets::{column, row};

use crate::{Card, Desk, Listed, Reach};

pub const CORNER: &str = "animate.cards.corner";
const WIDTH: f32 = 288.0;
const EDGE: f32 = 12.0;
const GLYPH: f32 = 16.0;

pub fn cards(desk: Desk<'_>) -> Option<Element> {
    if !desk.is_easy_mode() || desk.transport().is_playing() {
        return None;
    }
    let (pins, hands): (Vec<Listed>, Vec<Listed>) = desk
        .listed_holds()
        .into_iter()
        .partition(|listed| matches!(listed.hold.holder, Holder::Pin { .. }));
    let mut stack = Vec::new();
    if let (Some(card), Some(body), Some(pose_sample)) =
        (desk.card(), desk.chosen_body().cloned(), desk.sampled_now())
    {
        stack.push(match card {
            Card::Face(tab) => face::face(desk, &body, &pose_sample, tab),
            Card::Hand(side) => {
                let hand_holds: Vec<Listed> = hands
                    .into_iter()
                    .filter(|listed| listed.hold.holder == Holder::Hand(side))
                    .collect();
                hand::hand(desk, &pose_sample, side, &hand_holds)
            }
        });
    }
    if !pins.is_empty() {
        let pinned = frame(title(tr!("animate.hold.pins")), holds::holds(desk, &pins));
        stack.push(pinned.build());
    }
    if stack.is_empty() {
        return None;
    }
    Some(
        column_of(stack)
            .gap(Space::SNUG)
            .width(WIDTH)
            .build()
            .anchored(Anchor::new(CORNER, AnchorEdge::BelowEnd).offset(-EDGE, EDGE)),
    )
}

fn unit_slider(value: f32) -> Slider {
    slider(value)
        .range(0.0..=1.0)
        .control_size(ControlSize::Small)
}

fn amount(reach: Reach, value: f32, set: impl Fn(&mut PoseSample, f32) + 'static) -> impl View {
    unit_slider(value)
        .on_change(reach.act_with(move |studio, value: f32| {
            studio.touch_with(&mut |pose_sample| set(pose_sample, value));
        }))
        .fill_width()
}

fn strength(reach: Reach, expression: Expression, value: f32) -> impl View {
    unit_slider(value)
        .on_change(reach.act_with(move |studio, value: f32| {
            studio.express(&mut |faces, weights| faces.express(weights, expression, value));
        }))
        .fill_width()
}

fn title(label: Label) -> Text {
    text(label).bold().no_wrap()
}

fn card(reach: Reach, label: Label, body: impl View) -> impl View {
    let header = row! {
        title(label).grow(1.0);
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(GLYPH)))
            .icon_only()
            .small()
            .ghost()
            .focusless()
            .on_press(reach.act(move |studio| studio.open_card(None)))
            .tooltip(tr!("animate.tip.close"));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width();
    frame(header, body)
}

fn frame(header: impl View, body: impl View) -> impl View {
    let palette = palette();
    container(
        column! {
            header;
            body;
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .background(palette.surface)
    .border(1.0, palette.outline)
    .radius(Corner::Panel)
    .padding(Inset::all(EDGE))
    .fill_width()
}
