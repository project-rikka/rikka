mod grab;
mod history;
mod layers;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::paint::Paint;
use crate::session;
use crate::tab::Tab;
use crate::view::SIDE_PANEL;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Panel {
    #[default]
    Layers,
    History,
}

const PANELS: [Panel; 2] = [Panel::Layers, Panel::History];

pub(crate) fn side(paint: &Paint, tab: &Tab) -> Element {
    if let Some(session) = tab.session() {
        return session::panel(tab, session);
    }
    let current = paint.panel();
    let at = PANELS
        .iter()
        .position(|panel| *panel == current)
        .unwrap_or(0);
    let body = match current {
        Panel::Layers => layers::list(paint, tab).any(),
        Panel::History => history::list(tab).any(),
    };
    let content = column! {
        container(
            segmented(at)
                .segments([tr!("paint.layers.title"), tr!("paint.history.title")])
                .focusless()
                .on_select(|paint: &mut Paint, at: usize| paint.show_panel(PANELS[at])),
        )
        .padding(Space::SNUG)
        .fill_width();
        page_stack(at, body).grow(1.0).fill_width();
    }
    .fill();
    container(content)
        .background(palette().surface)
        .width(SIDE_PANEL)
        .fill_height()
        .any()
}
