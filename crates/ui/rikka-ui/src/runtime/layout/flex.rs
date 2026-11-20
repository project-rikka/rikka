use crate::runtime::scratch::{Scratch, pooled};
use crate::style::Layout;

use super::edges::{self, Edges, clamp, length, sized};

pooled! {
    Item => ITEMS,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Item {
    pub(super) basis: f32,
    pub(super) inner_basis: f32,
    pub(super) least: f32,
    pub(super) most: Option<f32>,
    pub(super) hypothetical: f32,
    pub(super) grow: f32,
    pub(super) shrink: f32,
    pub(super) margin: f32,
    pub(super) target: f32,
    frozen: bool,
    violation: f32,
}

impl Item {
    pub(super) fn new(
        style: &Layout,
        edges: &Edges,
        axis: usize,
        room: Option<f32>,
        content: f32,
        min_content: f32,
    ) -> Self {
        let pb = edges.pad[axis];
        let own = sized(length(style.size, axis), room);
        let basis = sized(style.basis, room).or(own).unwrap_or(content).max(pb);
        let most = sized(length(style.max, axis), room);
        let least = automatic_min(style, axis, room, min_content, pb);
        let hypothetical = clamp(basis, Some(least.max(pb)), most);
        Self {
            basis,
            inner_basis: basis - pb,
            least,
            most,
            hypothetical,
            grow: style.grow,
            shrink: style.shrink,
            margin: edges.margin[axis],
            target: hypothetical,
            frozen: false,
            violation: 0.0,
        }
    }

    pub(super) fn outer(&self) -> f32 {
        self.hypothetical + self.margin
    }
}

pub(super) fn automatic_min(
    style: &Layout,
    axis: usize,
    room: Option<f32>,
    min_content: f32,
    pb: f32,
) -> f32 {
    if let Some(min) = sized(length(style.min, axis), room) {
        return min;
    }
    if edges::scrolls(style, axis) {
        return 0.0;
    }
    let own = sized(length(style.size, axis), room);
    let most = sized(length(style.max, axis), room);
    let mut least = min_content;
    if let Some(own) = own {
        least = least.min(own);
    }
    if let Some(most) = most {
        least = least.min(most);
    }
    least.max(pb)
}

pub(super) fn room(items: &[Item], inner: f32, gap: f32, scrolls: bool) -> f32 {
    if !scrolls {
        return inner;
    }
    let natural = edges::gaps(gap, items.len()) + items.iter().map(Item::outer).sum::<f32>();
    inner.max(natural)
}

pub(super) fn resolve(items: &mut [Item], inner: f32, gap: f32) {
    let gaps = edges::gaps(gap, items.len());
    let hypothetical: f32 = items.iter().map(Item::outer).sum();
    let used_flex_factor = gaps + hypothetical;
    let growing = used_flex_factor < inner;
    let shrinking = used_flex_factor > inner;
    let exactly = !growing && !shrinking;

    for item in items.iter_mut() {
        item.target = item.hypothetical;
        item.frozen = exactly
            || (item.grow == 0.0 && item.shrink == 0.0)
            || (growing && item.basis > item.hypothetical)
            || (shrinking && item.basis < item.hypothetical);
    }
    if exactly {
        return;
    }

    let used = |items: &[Item]| -> f32 {
        gaps + items
            .iter()
            .map(|item| match item.frozen {
                true => item.target + item.margin,
                false => item.basis + item.margin,
            })
            .sum::<f32>()
    };
    let initial_free = inner - used(items);

    loop {
        if items.iter().all(|item| item.frozen) {
            break;
        }
        let used_space = used(items);
        let (grows, shrinks) = items
            .iter()
            .filter(|item| !item.frozen)
            .fold((0.0, 0.0), |(grows, shrinks), item| {
                (grows + item.grow, shrinks + item.shrink)
            });
        let free = if growing && grows < 1.0 {
            (initial_free * grows - gaps).min(inner - used_space)
        } else if shrinking && shrinks < 1.0 {
            (initial_free * shrinks - gaps).max(inner - used_space)
        } else {
            inner - used_space
        };

        if free.is_normal() {
            if growing && grows > 0.0 {
                for item in items.iter_mut().filter(|item| !item.frozen) {
                    item.target = item.basis + free * (item.grow / grows);
                }
            } else if shrinking && shrinks > 0.0 {
                let scaled: f32 = items
                    .iter()
                    .filter(|item| !item.frozen)
                    .map(|item| item.inner_basis * item.shrink)
                    .sum();
                if scaled > 0.0 {
                    for item in items.iter_mut().filter(|item| !item.frozen) {
                        let factor = item.inner_basis * item.shrink;
                        item.target = item.basis + free * (factor / scaled);
                    }
                }
            }
        }

        let mut total = 0.0;
        for item in items.iter_mut().filter(|item| !item.frozen) {
            let clamped = clamp(item.target, Some(item.least), item.most).max(0.0);
            item.violation = clamped - item.target;
            item.target = clamped;
            total += item.violation;
        }
        for item in items.iter_mut().filter(|item| !item.frozen) {
            item.frozen = match total {
                total if total > 0.0 => item.violation > 0.0,
                total if total < 0.0 => item.violation < 0.0,
                _ => true,
            };
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Contribution<'a> {
    pub(super) style: &'a Layout,
    pub(super) edges: &'a Edges,
    pub(super) axis: usize,
    pub(super) least: f32,
    pub(super) content: f32,
    pub(super) measured: f32,
}

pub(super) fn contribution(of: Contribution<'_>, row: bool, container_inset: f32) -> f32 {
    let Contribution {
        style,
        edges,
        axis,
        least,
        content,
        measured,
    } = of;
    let pb = edges.pad[axis];
    let margin = edges.margin[axis];
    let preferred = length(style.size, axis).fixed();
    let style_min = length(style.min, axis).fixed();
    let style_max = length(style.max, axis).fixed();
    let basis = style.basis.fixed().or(preferred).unwrap_or(content).max(pb);
    let inner_basis = basis - pb;
    let resolved_min = automatic_min(style, axis, None, least, pb);
    let clamping = Some(preferred.map_or(basis, |preferred| basis.max(preferred)));
    let basis_min = clamping.filter(|_| style.shrink == 0.0);
    let basis_max = clamping.filter(|_| style.grow == 0.0);
    let min_main = match (style_min, basis_min) {
        (Some(min), Some(floor)) => min.max(floor),
        (Some(min), None) => min,
        (None, Some(floor)) => floor,
        (None, None) => resolved_min,
    }
    .max(resolved_min);
    let max_main = match (style_max, basis_max) {
        (Some(max), Some(ceiling)) => max.min(ceiling),
        (Some(max), None) => max,
        (None, Some(ceiling)) => ceiling,
        (None, None) => f32::INFINITY,
    };
    let scroller = style.scrolls.is_some();
    let contribution = match preferred {
        Some(preferred) if max_main <= min_main || max_main <= preferred => {
            preferred.min(max_main).max(min_main) + margin
        }
        _ if max_main <= min_main => min_main + margin,
        _ if scroller => basis + margin,
        _ => {
            let measured = measured + margin;
            match row {
                true => clamp(measured, style_min, style_max).max(container_inset),
                false => clamp(measured.max(basis), style_min, style_max).max(container_inset),
            }
        }
    };
    let diff = contribution - basis;
    let fraction = if diff > 0.0 {
        diff / style.grow.max(1.0)
    } else if diff < 0.0 {
        diff / (style.shrink * inner_basis).max(1.0)
    } else {
        0.0
    };
    let share = if fraction > 0.0 {
        style.grow.max(1.0) * fraction
    } else if fraction < 0.0 {
        style.shrink.max(1.0) * inner_basis * fraction
    } else {
        0.0
    };
    basis + share
}

pub(super) fn broken(outer: &[f32], limit: f32, gap: f32) -> Scratch<usize> {
    let mut bounds = Scratch::new();
    bounds.push(0);
    let mut length = 0.0;
    let mut first = 0;
    for (index, size) in outer.iter().enumerate() {
        length += size + if index == first { 0.0 } else { gap };
        if length > limit && index != first {
            bounds.push(index);
            first = index;
            length = *size;
        }
    }
    bounds.push(outer.len());
    bounds
}

pub(super) fn unbroken(count: usize) -> Scratch<usize> {
    let mut bounds = Scratch::new();
    bounds.extend([0, count]);
    bounds
}
