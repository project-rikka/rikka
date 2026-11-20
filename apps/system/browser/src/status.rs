use rikka_app::prelude::*;
use rikka_web::Tooltip;

use crate::requests::Mark;
use crate::shell::Shell;
use browser_model::TabId;

const WIDEST: f32 = 480.0;
const MARGIN: f32 = 6.0;
const BELOW_POINTER: f32 = 20.0;

impl Shell {
    pub(crate) fn page_tooltip(&mut self, tab: TabId, tooltip: Option<Tooltip>) {
        if let Some(tab_entry) = self.browser().edit().tab_mut(tab) {
            tab_entry.meta.tooltip.set(tooltip);
        }
    }
}

pub(crate) fn hovered(shell: &Shell, tab: TabId) -> Option<Element> {
    let link = (*shell.model().tab(tab)?.meta.hovered).clone()?;
    let pill = container(text(link).caption().no_wrap().shrink(1.0).min_width(0.0))
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .background(palette().surface)
        .border(1.0, palette().outline.with_alpha(0.5))
        .radius(Corner::Control)
        .max_width(WIDEST)
        .build();
    Some(
        crate::gestures::inert(
            column_of([spacer().grow(1.0).any(), pill])
                .padding(Inset::all(MARGIN))
                .fill(),
        )
        .position(0.0, 0.0),
    )
}

pub(crate) fn tooltip(shell: &Shell, tab: TabId, mark: Mark) -> Option<Element> {
    let tip = (*shell.model().tab(tab)?.meta.tooltip).clone()?;
    let card = container(text(tip.text).caption())
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .background(palette().surface)
        .border(1.0, palette().outline.with_alpha(0.5))
        .radius(Corner::Control)
        .max_width(WIDEST)
        .build();
    Some(
        card.anchored(
            Anchor::new(mark, AnchorEdge::BelowStart)
                .offset(tip.x as f32, tip.y as f32 + BELOW_POINTER),
        ),
    )
}
