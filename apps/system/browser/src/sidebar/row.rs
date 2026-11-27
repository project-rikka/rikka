use rikka_app::prelude::*;
use rikka_ui::{DragEnd, Setter};
use rikka_widgets::row;

use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::sidebar::face::Face;
use crate::sidebar::look::{FAVICON, INDENT, ROW};
use crate::widgets::{DragItem, favicon, glyph_button, grip};
use browser_model::{Row, Slot, Sound, TabId};

const SMALL: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowTarget {
    Tab(TabId),
    Slot(Slot),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    pub(crate) face: Face,
    pub(crate) depth: usize,
    pub(crate) opens: RowTarget,
    pub(crate) subject: Subject,
    pub(crate) row: Row,
    pub(crate) tears_off: bool,
}

pub(crate) fn row(line: Line) -> Element {
    local_state(false, move |hovered: bool, set: Setter<bool>| {
        drawn(line, hovered, set)
    })
}

fn drawn(line: Line, hovered: bool, set: Setter<bool>) -> Element {
    let Line {
        face,
        depth,
        opens,
        subject,
        row,
        tears_off,
    } = line;
    let leading = match face.loading {
        true => spinner().extent(Length::px(FAVICON)).any(),
        false => crate::memory_saver::dimmed(
            favicon(face.favicon.as_ref(), face.symbol, FAVICON),
            face.discarded,
        ),
    };
    let dim = face.tab.is_none() && !face.lit;
    let title = text(face.title.clone())
        .label()
        .no_wrap()
        .grow(1.0)
        .min_width(0.0);
    let title = match dim {
        true => title.muted().any(),
        false => title.any(),
    };
    let closable = face.tab.filter(|_| hovered || face.lit);
    let body = row! {
        spacer().width(depth as f32 * INDENT);
        leading;
        title;
        sound(face.sound, face.tab);
        closable.map(close);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .fill();
    let carried = DragItem {
        row,
        symbol: face.symbol,
        label: face.title.clone(),
    };
    let mut gripped = grip(body)
        .highlighted(face.lit)
        .on_press(move |shell: &mut Shell| match opens {
            RowTarget::Tab(tab) => shell.select(tab),
            RowTarget::Slot(slot) => shell.select_slot(slot),
        })
        .aim(move |menus: &mut Menus| menus.aim(subject.clone()))
        .hover(move |inside: bool| {
            set(inside);
            skip_view();
        })
        .drag_item(carried);
    if let Some(tab) = face.tab {
        gripped = gripped.middle(move |shell: &mut Shell| shell.close(tab));
        if tears_off {
            gripped = gripped.ended(move |shell: &mut Shell, end: DragEnd| {
                if end == DragEnd::Nowhere {
                    shell.tear_off(tab);
                }
            });
        }
    }
    gripped.height(ROW).fill_width().any()
}

fn sound(sound: Sound, tab: Option<TabId>) -> Option<Element> {
    let tab = tab?;
    let symbol = match sound {
        Sound::Playing => MaterialSymbol::VOLUME_UP,
        Sound::Muted => MaterialSymbol::VOLUME_OFF,
        Sound::Silent => return None,
    };
    Some(
        glyph_button(symbol, SMALL)
            .on_press(move |shell: &mut Shell| shell.toggle_mute(tab))
            .any(),
    )
}

fn close(tab: TabId) -> Element {
    glyph_button(MaterialSymbol::CLOSE, SMALL)
        .on_press(move |shell: &mut Shell| shell.close(tab))
        .any()
}
