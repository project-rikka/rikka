use animate_motion::Tag;
use animate_ui::{choice, heading, labelled};
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::{Desk, wording};

const COLUMNS: usize = 3;
const PARTS_APART: &str = " · ";

pub(super) fn key(desk: Desk<'_>) -> impl View {
    let shown_tag = desk.shown_tag();
    let unregistered: Vec<String> = desk
        .unregistered_parts()
        .into_iter()
        .map(|part| animate_ui::part_label(part).as_str().to_owned())
        .chain(desk.unregistered_morphs())
        .collect();
    column! {
        heading(tr!("animate.easy.key"));
        grid_of(
            COLUMNS,
            Tag::ALL.map(|tag| {
                choice(wording::tag(tag), tag == shown_tag)
                    .on_press(desk.act(move |studio| studio.retag(tag)))
                    .fill_width()
            }),
        )
        .gap(Space::TIGHT)
        .fill_width();
        if !unregistered.is_empty() {
            labelled(
                tr!("animate.easy.unregistered"),
                text(unregistered.join(PARTS_APART)).caption().color(palette().accent),
                None,
            );
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}
