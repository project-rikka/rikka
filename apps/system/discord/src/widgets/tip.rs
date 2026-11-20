use std::hash::Hash;

use rikka_app::prelude::*;

use crate::widgets::{Pointing, mark_of, pointing};

pub(crate) fn with_tooltip(subject: impl Hash, body: impl View, tip: impl Into<Label>) -> Element {
    let mark = mark_of(subject);
    let (body, tip) = (body.build().keyed(mark), tip.into());
    local_state(
        Pointing::Away,
        move |pointed: Pointing, set: Setter<Pointing>| {
            let mut layers = vec![body];
            if pointed == Pointing::Hovered {
                layers.push(
                    container(text(tip).caption().semibold())
                        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                        .background(palette().surface)
                        .border(1.0, palette().outline)
                        .radius(Corner::Item)
                        .max_width(240.0)
                        .build()
                        .anchored(Anchor::above(mark).offset(0.0, -Space::TIGHT.px())),
                );
            }
            pointing(column_of(layers), move |pointed: Pointing| {
                set(pointed);
                skip_view();
            })
            .size(Length::HUG, Length::HUG)
        },
    )
}
