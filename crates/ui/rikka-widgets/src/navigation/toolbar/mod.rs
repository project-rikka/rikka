mod button;
mod fit;
mod group;
mod item;
mod more;
mod slot;

use std::cell::RefCell;

use rikka_ui::{
    Align, Arranged, Behavior, Element, Inset, Layout, Length, Space, Style, Styled, View, Widget,
};

use crate::blocks::flex::row_of;
use crate::control::ControlSize;
use crate::navigation::menu::{MenuItem, menu_separator};
use crate::navigation::toolbar::fit::{Measure, Room};
use crate::navigation::toolbar::more::More;
use crate::navigation::toolbar::slot::{Placement, Slot, divider_space};

pub use button::{ToolButton, tool_button};
pub use group::{ToolGroup, ToolPriority, tool_group};
pub use item::{ToolItem, tool_item};

const CONTENT: u8 = 0;

#[must_use]
pub struct Toolbar {
    entries: Vec<Entry>,
    contents: RefCell<Option<Vec<Vec<Element>>>>,
    divided: bool,
    gap: f32,
    padding: Inset,
    size: ControlSize,
    style: Style,
}

enum Entry {
    Group {
        priority: ToolPriority,
        gap: Option<f32>,
        overflow: Vec<MenuItem>,
    },
    Flexible {
        least: f32,
    },
    Spacer,
}

pub fn toolbar() -> Toolbar {
    Toolbar {
        entries: Vec::new(),
        contents: RefCell::new(Some(Vec::new())),
        divided: false,
        gap: Space::SNUG.px(),
        padding: Inset::xy(Space::SNUG, Space::TIGHT),
        size: ControlSize::Small,
        style: Style::default(),
    }
}

impl Toolbar {
    pub fn group(self, group: ToolGroup) -> Self {
        if group.items.is_empty() {
            return self;
        }
        let mut overflow = Vec::new();
        let mut content = Vec::with_capacity(group.items.len());
        for item in group.items {
            overflow.extend(item.overflow);
            content.push(item.content);
        }
        self.push(
            Entry::Group {
                priority: group.priority,
                gap: group.gap,
                overflow,
            },
            content,
        )
    }

    pub fn spacer(self) -> Self {
        self.push(Entry::Spacer, Vec::new())
    }

    pub fn flexible(self, content: impl View, least: f32) -> Self {
        self.push(Entry::Flexible { least }, vec![content.build()])
    }

    pub fn divided(mut self) -> Self {
        self.divided = true;
        self
    }

    pub fn gap(mut self, gap: impl Into<Space>) -> Self {
        self.gap = gap.into().px();
        self
    }

    pub fn padding(mut self, padding: impl Into<Inset>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    fn push(mut self, entry: Entry, content: Vec<Element>) -> Self {
        self.entries.push(entry);
        self.contents
            .get_mut()
            .as_mut()
            .expect("contents are taken only by build")
            .push(content);
        self
    }

    fn groups(&self) -> Vec<bool> {
        self.entries
            .iter()
            .map(|entry| matches!(entry, Entry::Group { .. }))
            .collect()
    }

    fn divider(&self) -> Option<f32> {
        self.divided.then(|| divider_space(self.gap))
    }

    fn holder(&self, entry: &Entry, content: Vec<Element>) -> Element {
        let row = row_of(content).align_items(Align::Center);
        let row = match entry {
            Entry::Group { gap, .. } => row.gap(gap.unwrap_or(self.gap)),
            Entry::Flexible { .. } => row.gap(self.gap).fill_width(),
            Entry::Spacer => row,
        };
        row.build().keyed(CONTENT)
    }

    fn room(&self, room: f32) -> Room {
        Room {
            inner: (room - self.padding.horizontal()).max(0.0),
            gap: self.gap,
            divider: self.divider(),
            more: More::width(self.size),
        }
    }

    fn reprioritized(&self, measures: &[Measure]) -> bool {
        measures.len() == self.entries.len()
            && self
                .entries
                .iter()
                .zip(measures)
                .any(|(entry, measure)| match (entry, measure) {
                    (Entry::Group { priority, .. }, Measure::Group { priority: was, .. }) => {
                        priority != was
                    }
                    _ => false,
                })
    }

    fn measures(&self, measures: &[Measure]) -> Vec<Measure> {
        self.entries
            .iter()
            .zip(measures)
            .map(|(entry, measure)| match (entry, *measure) {
                (
                    Entry::Group { priority, .. },
                    Measure::Group {
                        width, overflows, ..
                    },
                ) => Measure::Group {
                    width,
                    priority: *priority,
                    overflows,
                },
                (_, measure) => measure,
            })
            .collect()
    }

    fn overflow(&self, hidden: &[bool]) -> Vec<MenuItem> {
        let mut items = Vec::new();
        for (entry, _) in self
            .entries
            .iter()
            .zip(hidden)
            .filter(|(_, hidden)| **hidden)
        {
            if let Entry::Group { overflow, .. } = entry
                && !overflow.is_empty()
            {
                items.push(menu_separator());
                items.extend(overflow.iter().cloned());
            }
        }
        items
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ToolbarState {
    hidden: Vec<bool>,
    divided: Vec<bool>,
    room: f32,
    measures: Vec<Measure>,
}

impl Widget for Toolbar {
    type State = ToolbarState;

    const ARRANGES: bool = true;

    fn layout(&self) -> Layout {
        Layout::row()
            .align_items(Align::Center)
            .gap_exact(self.gap, self.gap)
            .padding(self.padding)
            .size(Length::FILL, Length::HUG)
            .min_width(Length::px(0.0))
            .with(&self.style)
    }

    fn behavior(&self, _state: &ToolbarState) -> Behavior {
        Behavior::CLIPS
    }

    fn build(&self, state: &mut ToolbarState) -> Vec<Element> {
        if state.hidden.len() != self.entries.len() {
            state.hidden = vec![false; self.entries.len()];
        }
        if self.reprioritized(&state.measures) {
            state.measures = self.measures(&state.measures);
            state.hidden = fit::hidden(&state.measures, &self.room(state.room));
        }
        state.divided = fit::divided(&self.groups(), &state.hidden, self.divided);
        let mut fresh = self.contents.borrow_mut().take().map(Vec::into_iter);
        let mut children = Vec::with_capacity(self.entries.len() + 1);
        for (index, entry) in self.entries.iter().enumerate() {
            let content = fresh
                .as_mut()
                .map(|fresh| fresh.next().expect("one content per entry"));
            let placement = match entry {
                _ if state.hidden[index] => Placement::Offstage { x: state.room },
                Entry::Group { .. } => Placement::Fixed {
                    divider: state.divided[index].then(|| divider_space(self.gap)),
                },
                Entry::Flexible { least } => Placement::Flexible { least: *least },
                Entry::Spacer => Placement::Flexible { least: 0.0 },
            };
            let child = match (entry, content) {
                (Entry::Spacer, _) => None,
                (_, Some(content)) => Some(self.holder(entry, content)),
                (_, None) => Some(Element::retained(CONTENT)),
            };
            let slot = Element::with_children(Slot { placement }, child.into_iter().collect());
            children.push(slot.keyed(index));
        }
        let items = self.overflow(&state.hidden);
        if !items.is_empty() {
            children.push(
                Element::new(More {
                    items,
                    size: self.size,
                })
                .keyed(self.entries.len()),
            );
        }
        children
    }

    fn arranged(&self, state: &mut ToolbarState, arranged: Arranged<'_>) -> bool {
        let room = arranged.room().width();
        let laid: Vec<Measure> = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Group {
                    priority, overflow, ..
                } => {
                    let laid = arranged.child_rect(index).map_or(0.0, |rect| rect.width());
                    let divider = match state.divided.get(index) {
                        Some(true) => divider_space(self.gap),
                        _ => 0.0,
                    };
                    Measure::Group {
                        width: laid - divider,
                        priority: *priority,
                        overflows: !overflow.is_empty(),
                    }
                }
                Entry::Flexible { least } => Measure::Flexible { least: *least },
                Entry::Spacer => Measure::Flexible { least: 0.0 },
            })
            .collect();
        let hidden = fit::hidden(&laid, &self.room(room));
        let moved = room != state.room && hidden.contains(&true);
        let changed = hidden != state.hidden || moved;
        state.hidden = hidden;
        state.room = room;
        state.measures = laid;
        changed
    }
}

impl View for Toolbar {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl Styled for Toolbar {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl std::fmt::Debug for Toolbar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toolbar")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}
