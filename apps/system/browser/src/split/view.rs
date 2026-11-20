use rikka_app::prelude::*;
use rikka_widgets::{row, split};

use crate::content::tab_page;
use crate::internal::route::Presented;
use crate::model::Tab;
use crate::shell::Shell;
use crate::sidebar::titled;
use crate::split::drop_zones::{Side, edges, highlight};
use crate::split::focus::pane_focus;
use crate::widgets::{favicon, glyph_button};
use browser_model::Group;
use browser_model::{Row, TabId};

const HEADER: f32 = 28.0;
const GLYPH: f32 = 14.0;
const RING: f32 = 2.0;

#[derive(State)]
pub(crate) struct Panes {
    hovered_drop: Tracked<Option<(TabId, Side)>>,
}

impl Panes {
    pub(crate) fn new() -> Panes {
        Panes {
            hovered_drop: Tracked::new(None),
        }
    }

    fn hover(&mut self, tab: TabId, side: Option<Side>) {
        let hovered_drop = side.map(|side| (tab, side));
        self.hovered_drop.set(hovered_drop);
    }
}

impl std::fmt::Debug for Panes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Panes")
            .field("hovered_drop", &*self.hovered_drop)
            .finish()
    }
}

pub(crate) fn alone(shell: &Shell, tab_entry: &Tab) -> Element {
    accept_drops(tab_page(shell, tab_entry), tab_entry.id)
}

pub(crate) fn landing_mark(shell: &Shell, tab: TabId) -> Option<Element> {
    let (over, side) = (*shell.panes.hovered_drop)?;
    (over == tab).then(|| highlight(side))
}

pub(crate) fn view(shell: &Shell, group: &Group) -> Element {
    let model = shell.model();
    let focus = model.window(shell.id()).and_then(|layout| layout.active());
    let panes: Vec<Element> = group
        .panes()
        .iter()
        .filter_map(|tab| {
            let tab_entry = model.tab(*tab)?;
            Some(pane(shell, tab_entry, focus == Some(*tab)).keyed(("pane", *tab)))
        })
        .collect();
    nested(group, panes, 0)
}

fn nested(group: &Group, mut panes: Vec<Element>, divider: usize) -> Element {
    if panes.len() <= 1 {
        return panes.pop().unwrap_or_else(Element::empty);
    }
    let first = panes.remove(0);
    let id = group.id;
    split(
        group.arrangement.axis(),
        group.ratio(divider),
        first,
        nested(group, panes, divider + 1),
    )
    .on_resize(move |shell: &mut Shell, ratio: f32| shell.resize_split(id, divider, ratio))
    .fill()
    .any()
}

fn accept_drops(page: Element, tab: TabId) -> Element {
    edges(
        page,
        move |panes: &mut Panes, side: Option<Side>| panes.hover(tab, side),
        move |shell: &mut Shell, row: Row, side: Side| shell.dropped_beside(row, tab, side),
    )
}

fn pane(shell: &Shell, tab_entry: &Tab, focused: bool) -> Element {
    let tab = tab_entry.id;
    let ring = match focused {
        true => palette().accent,
        false => palette().outline.with_alpha(0.5),
    };
    let body = column_of([
        header(tab_entry, focused),
        tab_page(shell, tab_entry).grow(1.0).fill_width(),
    ])
    .fill();
    pane_focus(
        container(accept_drops(body.any(), tab))
            .border(RING, ring)
            .radius(Corner::Panel)
            .clipped()
            .fill(),
        tab,
    )
}

fn header(tab_entry: &Tab, focused: bool) -> Element {
    let tab = tab_entry.id;
    let title = match &*tab_entry.internal {
        Some(internal) => internal.title().to_string(),
        None if tab_entry.is_blank() => tr!("browser.tab.new").to_string(),
        None => titled(&tab_entry.meta.title, &tab_entry.meta.url),
    };
    let title = text(title).caption().no_wrap().grow(1.0).min_width(0.0);
    let title = match focused {
        true => title.any(),
        false => title.muted().any(),
    };
    row! {
        favicon(tab_entry.meta.favicon.as_ref(), MaterialSymbol::PUBLIC, GLYPH);
        title;
        glyph_button(MaterialSymbol::OPEN_IN_FULL, GLYPH)
            .on_press(move |shell: &mut Shell| shell.separate_pane(tab));
        glyph_button(MaterialSymbol::CLOSE, GLYPH)
            .on_press(move |shell: &mut Shell| shell.close(tab));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .background(palette().surface)
    .height(HEADER)
    .shrink(0.0)
    .fill_width()
    .any()
}
