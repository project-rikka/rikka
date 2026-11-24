use rikka_types::Size;
use rikka_ui::{Axis, Inset, Layout, Length, View};
use rikka_widgets::{Flex, column_of, row_of};

pub(crate) fn line(axis: Axis, children: impl IntoIterator<Item = impl View>) -> Flex {
    match axis {
        Axis::Horizontal => row_of(children),
        Axis::Vertical => column_of(children),
    }
}

#[must_use]
pub(crate) fn layout(axis: Axis) -> Layout {
    match axis {
        Axis::Horizontal => Layout::row(),
        Axis::Vertical => Layout::column(),
    }
}

#[must_use]
pub(crate) fn sized(layout: Layout, axis: Axis, along: Length, across: Length) -> Layout {
    match axis {
        Axis::Horizontal => layout.size(along, across),
        Axis::Vertical => layout.size(across, along),
    }
}

#[must_use]
pub(crate) fn across(layout: Layout, axis: Axis, across: Length) -> Layout {
    match axis {
        Axis::Horizontal => layout.height(across),
        Axis::Vertical => layout.width(across),
    }
}

#[must_use]
pub(crate) fn along_size(layout: Layout, axis: Axis, along: Length) -> Layout {
    match axis {
        Axis::Horizontal => layout.width(along),
        Axis::Vertical => layout.height(along),
    }
}

#[must_use]
pub(crate) fn padding(axis: Axis, along: f32, across: f32) -> Inset {
    match axis {
        Axis::Horizontal => Inset::xy(along, across),
        Axis::Vertical => Inset::xy(across, along),
    }
}

#[must_use]
pub(crate) fn inset(axis: Axis, before: f32, after: f32) -> Inset {
    match axis {
        Axis::Horizontal => Inset::exact(0.0, after, 0.0, before),
        Axis::Vertical => Inset::exact(before, 0.0, after, 0.0),
    }
}

#[must_use]
pub(crate) fn size(axis: Axis, along: f32, across: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(along, across),
        Axis::Vertical => Size::new(across, along),
    }
}
