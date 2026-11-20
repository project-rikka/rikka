use std::rc::Rc;

use rikka_types::{Key, Modifiers};
use rikka_ui::{Action, Element, Handler, Length, Style, Styled, View};

use crate::control::ControlSize;
use crate::lists::collection::{Collection, Lines, Props, Slot};
use crate::lists::list_item::{self, ListItem, Metrics};

enum Source {
    Items(Rc<[ListItem]>),
    Generated(usize, Rc<dyn Fn(usize) -> ListItem>),
}

impl Source {
    fn count(&self) -> usize {
        match self {
            Self::Items(items) => items.len(),
            Self::Generated(count, _) => *count,
        }
    }

    fn at(&self, index: usize) -> Option<ListItem> {
        match self {
            Self::Items(items) => items.get(index).cloned(),
            Self::Generated(count, item) => (index < *count).then(|| item(index)),
        }
    }

    fn with<R>(&self, index: usize, read: impl FnOnce(&ListItem) -> R) -> Option<R> {
        match self {
            Self::Items(items) => items.get(index).map(read),
            Self::Generated(count, item) => (index < *count).then(|| read(&item(index))),
        }
    }
}

impl Clone for Source {
    fn clone(&self) -> Self {
        match self {
            Self::Items(items) => Self::Items(Rc::clone(items)),
            Self::Generated(count, item) => Self::Generated(*count, Rc::clone(item)),
        }
    }
}

#[must_use]
pub struct List {
    rows: Source,
    metrics: Metrics,
    even: bool,
    lines: Lines,
    props: Props,
    style: Style,
}

pub fn list(items: impl IntoIterator<Item = ListItem>) -> List {
    let items: Rc<[ListItem]> = items.into_iter().collect::<Vec<_>>().into();
    let metrics = Metrics::measure(&items, ControlSize::default());
    List {
        even: !items.iter().any(ListItem::is_header),
        rows: Source::Items(items),
        metrics,
        lines: Lines::Down,
        props: chosen_and_hovered(),
        style: Style::default(),
    }
}

fn chosen_and_hovered() -> Props {
    Props {
        lit: true,
        ..Props::default()
    }
}

pub fn list_of(count: usize, item: impl Fn(usize) -> ListItem + 'static) -> List {
    let first = (count > 0).then(|| item(0));
    let metrics = Metrics::measure(first.as_slice(), ControlSize::default());
    List {
        rows: Source::Generated(count, Rc::new(item)),
        metrics,
        even: true,
        lines: Lines::Down,
        props: chosen_and_hovered(),
        style: Style::default(),
    }
}

impl List {
    pub fn count(&self) -> usize {
        self.rows.count()
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.metrics.size = size;
        self
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.lines = Lines::Grid(columns.max(1));
        self
    }

    pub fn tiles(mut self, width: Length) -> Self {
        self.lines = match width.fixed() {
            Some(tile) => Lines::Tiles(tile.max(1.0)),
            None => Lines::Down,
        };
        self
    }

    crate::lists::collection::collection_methods!();
}

impl std::fmt::Debug for List {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("List")
            .field("rows", &self.rows.count())
            .finish_non_exhaustive()
    }
}

impl View for List {
    fn build(self) -> Element {
        let List {
            rows,
            metrics,
            even,
            lines,
            mut props,
            style,
        } = self;
        if props.uniform.is_none() && even {
            props.uniform = Some(metrics.row_height());
        }
        props.estimate.get_or_insert(metrics.row_height());
        if props.reachable.is_none() {
            let rows = rows.clone();
            props.reachable = Some(Rc::new(move |index| {
                rows.with(index, ListItem::selectable).unwrap_or(false)
            }));
        }
        if props.named.is_none() {
            let rows = rows.clone();
            props.named = Some(Rc::new(move |index| {
                rows.with(index, ListItem::name).unwrap_or(0)
            }));
        }
        if !props.on_key.is_set() {
            let rows = rows.clone();
            props.on_key = Action::new(Handler::relaying(
                move |reach, (key, _, selected_index): (Key, Modifiers, Option<usize>)| {
                    let command = rows.with(selected_index?, |item| item.commanded(key))??;
                    command.call(reach, ());
                    Some(true)
                },
            ));
        }
        let count = rows.count();
        let cell = Rc::new(move |slot: Slot| match rows.at(slot.index) {
            Some(item) => list_item::row(item, metrics, slot.selected),
            None => Element::new(list_item::EmptyRow),
        });
        Collection::from_parts(lines, count, cell, props, style).build()
    }
}

impl Styled for List {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
