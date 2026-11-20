use std::rc::Rc;

use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::text::{Label, TextRun};
use rikka_ui::{Element, IconSource, Inset, Layout, Length, PaintCtx, Style, Styled, View, Widget};

use crate::control::ControlSize;
use crate::lists::collection::{Collection, Lines, Props, Slot};
use crate::rows::draw_cell;

const PADDING: f32 = 4.0;

const HEADER_HEIGHT: f32 = 0.8;
const HEADER_FONT: f32 = 0.85;

#[must_use]
pub struct NavItem {
    label: Label,
    icon: Option<IconSource>,
    tint: Option<Color>,
    header: bool,
}

pub fn nav_item(label: impl Into<Label>) -> NavItem {
    NavItem {
        label: label.into(),
        icon: None,
        tint: None,
        header: false,
    }
}

pub fn nav_header(label: impl Into<Label>) -> NavItem {
    NavItem {
        header: true,
        ..nav_item(label)
    }
}

impl NavItem {
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn tint(mut self, color: Color) -> Self {
        self.tint = Some(color);
        self
    }

    fn selectable(&self) -> bool {
        !self.header
    }
}

impl Clone for NavItem {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            icon: self.icon.clone(),
            tint: self.tint,
            header: self.header,
        }
    }
}

impl std::fmt::Debug for NavItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavItem")
            .field("label", &self.label)
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

#[must_use]
pub struct NavList {
    items: Vec<NavItem>,
    size: ControlSize,
    props: Props,
    style: Style,
}

pub fn nav_list(selected: usize) -> NavList {
    let props = Props {
        selected: Some(selected),
        lit: true,
        ..Props::default()
    };
    NavList {
        items: Vec::new(),
        size: ControlSize::default(),
        props,
        style: Style::default(),
    }
}

impl NavList {
    pub fn items(mut self, items: impl IntoIterator<Item = NavItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn item(mut self, item: NavItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    crate::lists::collection::collection_methods!();
}

impl std::fmt::Debug for NavList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavList")
            .field("selected", &self.props.selected)
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl View for NavList {
    fn build(self) -> Element {
        let NavList {
            items,
            size,
            mut props,
            style,
        } = self;
        let items: Rc<[NavItem]> = items.into();
        props.padding = Inset::all(PADDING);
        if props.reachable.is_none() {
            let rows = Rc::clone(&items);
            props.reachable = Some(Rc::new(move |index| {
                rows.get(index).is_some_and(NavItem::selectable)
            }));
        }
        let count = items.len();
        let cell = Rc::new(move |slot: Slot| {
            Element::new(Entry {
                item: items[slot.index].clone(),
                size,
                selected: slot.selected,
            })
        });
        Collection::from_parts(Lines::Down, count, cell, props, style).build()
    }
}

struct Entry {
    item: NavItem,
    size: ControlSize,
    selected: bool,
}

impl Widget for Entry {
    type State = ();

    fn layout(&self) -> Layout {
        let height = match self.item.header {
            true => self.size.height() * HEADER_HEIGHT,
            false => self.size.height(),
        };
        Layout::row()
            .size(Length::FILL, Length::px(height))
            .padding(Inset::NONE)
            .fixed()
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let row = ctx.bounds();
        let palette = ctx.theme().palette();
        let inset = self.size.gap();
        let font_size = self.size.font_size(ctx.theme());

        if self.item.header {
            let run = TextRun::new(self.item.label.clone(), font_size * HEADER_FONT)
                .medium()
                .no_wrap();
            let rect = Rect::new(
                row.x() + inset,
                row.max_y() - run.line_height,
                (row.width() - inset).max(0.0),
                run.line_height,
            );
            draw_cell(ctx, rect, rect.x(), rect.width(), run, palette.text_muted);
            return;
        }

        let foreground = match self.selected {
            true => palette.on_selection,
            false => palette.text_muted,
        };
        let mut label_x = row.x() + inset;
        if let Some(icon) = &self.item.icon {
            let size = self.size.icon_size();
            ctx.draw_icon_source(
                Rect::from_center(
                    Point::new(label_x + size / 2.0, row.center().y),
                    Size::new(size, size),
                ),
                icon,
                size,
                self.item.tint.unwrap_or(foreground),
            );
            label_x += size + self.size.gap();
        }

        let mut run = TextRun::new(self.item.label.clone(), font_size).no_wrap();
        if self.selected {
            run = run.medium();
        }
        let width = row.max_x() - inset - label_x;
        draw_cell(ctx, row, label_x, width, run, foreground);
    }

    fn same(&self, other: &Self) -> bool {
        self.item.label == other.item.label
            && self.item.icon == other.item.icon
            && self.item.tint == other.item.tint
            && self.item.header == other.item.header
            && self.size == other.size
            && self.selected == other.selected
    }

    fn debug_name(&self) -> &'static str {
        "NavItem"
    }
}

impl Styled for NavList {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
