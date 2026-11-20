use std::collections::HashSet;

use browser_store::SplitGroupId;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::memory_saver::dimmed;
use crate::menus::{Menus, Subject};
use crate::model::Model;
use crate::shell::Shell;
use crate::sidebar::{Face, rail_grip};
use crate::widgets::{DragItem, favicon, grip};
use browser_model::{Row, TabId, Target, WindowId};

const ICON: f32 = 14.0;
const MINI: f32 = 13.0;
const ROW: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Entry {
    Tab(TabId),
    Group(SplitGroupId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Entries {
    pub(crate) shown: Vec<Entry>,
    starts: Vec<usize>,
    count: usize,
}

impl Entries {
    pub(crate) fn collect(model: &Model, window: WindowId) -> Entries {
        let Some(layout) = model.window(window) else {
            return Entries::default();
        };
        let today = layout.today_here();
        let mut entries = Entries {
            count: today.len(),
            ..Entries::default()
        };
        for group in model.splits.in_space(window, layout.space()) {
            if !group.panes().iter().any(|pane| today.contains(pane)) {
                entries.shown.push(Entry::Group(group.id));
                entries.starts.push(0);
            }
        }
        let mut seen = HashSet::new();
        for (at, tab) in today.iter().enumerate() {
            let entry = match model.splits.containing(*tab) {
                Some(group) if group.window == window => Entry::Group(group.id),
                _ => Entry::Tab(*tab),
            };
            if seen.insert(entry) {
                entries.shown.push(entry);
                entries.starts.push(at);
            }
        }
        entries
    }

    pub(crate) fn today_index(&self, entry: usize) -> usize {
        self.starts.get(entry).copied().unwrap_or(self.count)
    }
}

impl Shell {
    pub(crate) fn dropped_in_today(&mut self, row: Row, entry: usize) {
        let at = Entries::collect(&self.model(), self.id()).today_index(entry);
        self.dropped(row, Target::Today(at));
        if let Row::Tab(tab) = row {
            self.browser().edit().gather_split(self.id(), tab);
        }
    }
}

struct Look {
    title: String,
    faces: Vec<Face>,
    lit: bool,
    focus: TabId,
    carried: TabId,
}

fn look(shell: &Shell, id: SplitGroupId) -> Option<Look> {
    let model = shell.model();
    let layout = model.window(shell.id())?;
    let group = model.splits.get(id)?;
    let active = layout.active();
    let faces: Vec<Face> = group
        .panes()
        .iter()
        .filter_map(|tab| Some(Face::from_tab(model.tab(*tab)?, active == Some(*tab))))
        .collect();
    let title = faces
        .iter()
        .map(|face| face.title.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let today = layout.today_here();
    let carried = group
        .panes()
        .iter()
        .copied()
        .filter(|pane| today.contains(pane))
        .min_by_key(|pane| today.iter().position(|tab| tab == pane))
        .unwrap_or(group.focus());
    Some(Look {
        title,
        lit: faces.iter().any(|face| face.lit),
        faces,
        focus: group.focus(),
        carried,
    })
}

pub(crate) fn line(shell: &Shell, id: &SplitGroupId) -> Element {
    let Some(look) = look(shell, *id) else {
        return Element::empty();
    };
    let icons = row_of(look.faces.iter().map(|face| {
        dimmed(
            favicon(face.favicon.as_ref(), face.symbol, ICON),
            face.discarded,
        )
    }))
    .gap(Space::HAIR)
    .align_items(Align::Center);
    let title = text(look.title.clone())
        .label()
        .no_wrap()
        .grow(1.0)
        .min_width(0.0);
    let title = match look.lit {
        true => title.any(),
        false => title.muted().any(),
    };
    let body = row! {
        icons;
        title;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .fill();
    let focus = look.focus;
    grip(body)
        .highlighted(look.lit)
        .on_press(move |shell: &mut Shell| shell.select(focus))
        .aim(move |menus: &mut Menus| menus.aim(Subject::Tab(focus)))
        .drag_item(DragItem {
            row: Row::Tab(look.carried),
            symbol: MaterialSymbol::SPLITSCREEN,
            label: look.title,
        })
        .height(ROW)
        .fill_width()
        .any()
}

pub(crate) fn cell(shell: &Shell, id: &SplitGroupId) -> Element {
    let Some(look) = look(shell, *id) else {
        return Element::empty();
    };
    let minis = look.faces.iter().map(|face| {
        dimmed(
            favicon(face.favicon.as_ref(), face.symbol, MINI),
            face.discarded,
        )
    });
    let mut minis: Vec<Element> = minis.collect();
    let marks = match minis.len() {
        0..=2 => row_of(minis).gap(Space::HAIR).any(),
        _ => {
            let below = minis.split_off(2);
            column_of([
                row_of(minis).gap(Space::HAIR).any(),
                row_of(below).gap(Space::HAIR).any(),
            ])
            .gap(Space::HAIR)
            .any()
        }
    };
    let focus = look.focus;
    rail_grip(marks, look.title.clone())
        .highlighted(look.lit)
        .on_press(move |shell: &mut Shell| shell.select(focus))
        .aim(move |menus: &mut Menus| menus.aim(Subject::Tab(focus)))
        .drag_item(DragItem {
            row: Row::Tab(look.carried),
            symbol: MaterialSymbol::SPLITSCREEN,
            label: look.title,
        })
        .any()
}
