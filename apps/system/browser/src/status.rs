use rikka_app::prelude::*;

use crate::shell::Shell;
use browser_model::TabId;

const WIDEST: f32 = 480.0;
const MARGIN: f32 = 6.0;

pub(crate) fn hovered(shell: &Shell, tab: TabId) -> Option<Element> {
    let link = (*shell.model().tab(tab)?.meta.hovered).clone()?;
    let pill = container(text(link).caption().no_wrap().shrink(1.0).min_width(0.0))
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .background(palette().surface)
        .border(1.0, palette().outline.with_alpha(0.5))
        .radius(Corner::Control)
        .max_width(WIDEST)
        .build();
    Some(crate::gestures::inert(
        column_of([spacer().grow(1.0).any(), pill])
            .padding(Inset::all(MARGIN))
            .fill(),
    ))
}
