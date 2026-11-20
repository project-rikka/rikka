use rikka_ui::{
    Align, Element, Flow, Inset, Justify, Layout, Space, Style, Styled, Track, Tracks, View, Widget,
};

use crate::dsl::built;

#[must_use]
pub struct Flex {
    flow: Flow,
    children: Vec<Element>,
    gap: f32,
    padding: Inset,
    align: Option<Align>,
    justify: Option<Justify>,
    wrap: bool,
    style: Style,
}

pub fn row_of(children: impl IntoIterator<Item = impl View>) -> Flex {
    Flex::new(Flow::Row, built(children))
}

pub fn column_of(children: impl IntoIterator<Item = impl View>) -> Flex {
    Flex::new(Flow::Column, built(children))
}

pub fn grid_of(columns: usize, children: impl IntoIterator<Item = impl View>) -> Flex {
    Flex::new(
        Flow::Grid(Tracks::new(Track::Even(Tracks::lines(columns)))),
        built(children),
    )
}

impl Flex {
    fn new(flow: Flow, children: Vec<Element>) -> Self {
        Self {
            flow,
            children,
            gap: 0.0,
            padding: Inset::NONE,
            align: None,
            justify: None,
            wrap: false,
            style: Style::default(),
        }
    }

    pub fn push(mut self, child: impl View) -> Self {
        self.children.extend(built([child]));
        self
    }

    pub fn extend(mut self, children: impl IntoIterator<Item = impl View>) -> Self {
        self.children.extend(built(children));
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

    pub fn align_items(mut self, align: Align) -> Self {
        self.align = Some(align);
        self
    }

    pub fn centered(self) -> Self {
        self.align_items(Align::Center)
    }

    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = Some(justify);
        self
    }

    pub fn wrap_items(mut self) -> Self {
        self.wrap = true;
        self
    }
}

impl Widget for Flex {
    type State = ();

    fn layout(&self) -> Layout {
        let mut layout = Layout {
            flow: self.flow,
            wrap: self.wrap,
            align_items: self.align,
            justify: self.justify,
            ..Layout::DEFAULT
        }
        .padding(self.padding)
        .gap_exact(self.gap, self.gap);
        self.style.apply(&mut layout);
        layout
    }

    fn same(&self, other: &Self) -> bool {
        self.flow == other.flow
            && self.gap == other.gap
            && self.padding == other.padding
            && self.align == other.align
            && self.justify == other.justify
            && self.wrap == other.wrap
            && self.style == other.style
    }

    fn debug_name(&self) -> &'static str {
        match self.flow {
            Flow::Column => "Column",
            Flow::Grid(_) => "Grid",
            Flow::Row => "Row",
        }
    }
}

impl std::fmt::Debug for Flex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flex")
            .field("children", &self.children.len())
            .finish_non_exhaustive()
    }
}

impl View for Flex {
    fn build(mut self) -> Element {
        let children = std::mem::take(&mut self.children);
        Element::with_children(self, children)
    }
}

impl Styled for Flex {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
