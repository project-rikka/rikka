use crate::style::{Align, Axis, Extent, Layout, Length};

pub(super) const X: usize = 0;
pub(super) const Y: usize = 1;

pub(super) fn length(extent: Extent, axis: usize) -> Length {
    match axis {
        X => extent.width,
        _ => extent.height,
    }
}

pub(super) fn sized(length: Length, basis: Option<f32>) -> Option<f32> {
    length
        .fixed()
        .or_else(|| basis.and_then(|basis| length.resolve(basis)))
}

pub(super) fn clamp(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let value = max.map_or(value, |max| value.min(max));
    min.map_or(value, |min| value.max(min))
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Edges {
    pub(super) start: [f32; 2],
    pub(super) pad: [f32; 2],
    pub(super) inset: [f32; 2],
    pub(super) end: [f32; 2],
    pub(super) margin: [f32; 2],
    pub(super) margin_start: [f32; 2],
    pub(super) gutter: [f32; 2],
}

impl Edges {
    pub(super) fn new(layout: &Layout, lane: f32) -> Self {
        let padding = layout.padding;
        let gutter = match layout.scrolls {
            Some(Axis::Vertical) => [lane, 0.0],
            Some(Axis::Horizontal) => [0.0, lane],
            None => [0.0, 0.0],
        };
        let pad = [padding.horizontal(), padding.vertical()];
        Self {
            start: [padding.left(), padding.top()],
            pad,
            inset: [pad[X] + gutter[X], pad[Y] + gutter[Y]],
            end: [padding.right(), padding.bottom()],
            margin: [layout.margin.horizontal(), layout.margin.vertical()],
            margin_start: [layout.margin.left(), layout.margin.top()],
            gutter,
        }
    }
}

pub(super) fn scrolls(layout: &Layout, axis: usize) -> bool {
    matches!(
        (layout.scrolls, axis),
        (Some(Axis::Horizontal), X) | (Some(Axis::Vertical), Y)
    )
}

pub(super) fn stretches(child: &Layout, parent: &Layout, axis: usize) -> bool {
    self_align(child, parent) == Align::Stretch && length(child.size, axis) == Length::HUG
}

pub(super) fn self_align(child: &Layout, parent: &Layout) -> Align {
    child
        .align_self
        .or(parent.align_items)
        .unwrap_or(Align::Stretch)
}

pub(super) fn gaps(gap: f32, count: usize) -> f32 {
    match count {
        0 | 1 => 0.0,
        _ => gap * (count - 1) as f32,
    }
}

pub(super) fn gap(layout: &Layout, axis: usize) -> f32 {
    match axis {
        X => layout.gap.width,
        _ => layout.gap.height,
    }
}

pub(super) fn main_axis(layout: &Layout) -> usize {
    match layout.flow {
        crate::style::Flow::Column => Y,
        crate::style::Flow::Row | crate::style::Flow::Grid(_) => X,
    }
}
