use cut_project::{Clip, Tween};
use rikka_app::prelude::*;

use super::rows::{easing_row, group_with};
use crate::cut::{Cut, Endpoint};

const EASING: &str = "cut-tween-easing";

pub(crate) fn tween(clip: &Clip, frame: i64) -> impl View {
    let tween = clip.tween();
    let moving = switch(tween.is_some())
        .control_size(ControlSize::Small)
        .enabled(clip.can_tween())
        .on_toggle(|cut: &mut Cut, on: bool| cut.set_tween(on))
        .build();
    let rows = match tween {
        Some(tween) => vec![
            endpoints(&tween, frame),
            easing_row(
                tr!("cut.tween.easing"),
                (EASING, clip.id()),
                tween.easing,
                |cut: &mut Cut, anchor: Anchor| cut.open_tween_easing(anchor),
            ),
            button(text(tr!("cut.tween.reverse")).no_wrap())
                .small()
                .ghost()
                .focusless()
                .on_press(Cut::reverse_tween)
                .fill_width()
                .build(),
        ],
        None => Vec::new(),
    };
    group_with(tr!("cut.tween.group"), Some(moving), rows)
}

fn endpoints(tween: &Tween, frame: i64) -> Element {
    let buttons = Endpoint::ALL.map(|endpoint| {
        let label = match endpoint {
            Endpoint::Start => tr!("cut.tween.start"),
            Endpoint::End => tr!("cut.tween.end"),
        };
        let button = button(text(label).no_wrap())
            .small()
            .focusless()
            .on_press(move |cut: &mut Cut| cut.go_to_endpoint(endpoint));
        match endpoint.frame(tween) == frame {
            true => button.secondary(),
            false => button.ghost(),
        }
        .grow(1.0)
        .build()
    });
    row_of(buttons).gap(Space::SNUG).fill_width().build()
}
