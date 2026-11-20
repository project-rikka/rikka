use std::sync::Arc;

use crate::front;
use crate::shapes::{ShardedLruCache, shaped};
use crate::shaping::{LayoutRequest, TextShape};
use crate::text_layout::TextLayout;

const CACHE_BUDGET: usize = 24 * 1024 * 1024;

const MAX_LAYOUTS_PER_KEY: usize = 6;

const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

type Layouts = Vec<Arc<TextLayout>>;

static LAYOUT_CACHE: ShardedLruCache<Layouts> = ShardedLruCache::new(CACHE_BUDGET);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reuse {
    Compatible,
    Nearest,
}

#[must_use]
pub fn layout(shape: TextShape<'_>, width: Option<f32>) -> Arc<TextLayout> {
    layout_with(shape, width, 0.0, Reuse::Compatible)
}

pub(crate) fn layout_spaced(
    shape: TextShape<'_>,
    letter_spacing: f32,
    width: Option<f32>,
) -> Arc<TextLayout> {
    layout_with(shape, width, letter_spacing, Reuse::Compatible)
}

#[must_use]
pub fn layout_approx(shape: TextShape<'_>, width: Option<f32>) -> Arc<TextLayout> {
    layout_with(shape, width, 0.0, Reuse::Nearest)
}

#[must_use]
pub fn recent_layout(shape: TextShape<'_>, width: Option<f32>) -> Option<Arc<TextLayout>> {
    let (key, request) = layout_key(&shape, width, 0.0);
    recall(front_key(key, width), &shape, &request)
}

fn layout_key(
    shape: &TextShape<'_>,
    width: Option<f32>,
    letter_spacing: f32,
) -> (u64, LayoutRequest) {
    let request = shape.layout_request(width, letter_spacing);
    (request.digest(shape.shape_digest()), request)
}

fn matching<'a>(
    widths: &'a Layouts,
    shape: &'a TextShape<'_>,
    request: &'a LayoutRequest,
) -> impl Iterator<Item = &'a Arc<TextLayout>> {
    widths
        .iter()
        .filter(move |layout| layout.request().is_compatible(request) && layout.shaped().is(shape))
}

fn recall(
    front_key: u64,
    shape: &TextShape<'_>,
    request: &LayoutRequest,
) -> Option<Arc<TextLayout>> {
    front::recall(front_key, |layout| {
        layout.request().is_compatible(request)
            && layout.serves(request.width, false)
            && layout.shaped().is(shape)
    })
}

fn lookup(
    key: u64,
    shape: &TextShape<'_>,
    request: &LayoutRequest,
    reuse: Reuse,
) -> Option<Arc<TextLayout>> {
    let width = request.width;
    let front_key = front_key(key, width);
    if let Some(found) = recall(front_key, shape, request) {
        return Some(found);
    }
    let mut shard = LAYOUT_CACHE.shard(key);
    let widths = shard.get(key)?;
    let bits = width.map(f32::to_bits);
    let exact =
        || matching(widths, shape, request).find(|layout| layout.width().map(f32::to_bits) == bits);
    let compatible = || matching(widths, shape, request).find(|layout| layout.serves(width, false));
    let near = || {
        matching(widths, shape, request)
            .filter(|layout| layout.serves(width, true))
            .min_by(|one, other| {
                let gap = |layout: &Arc<TextLayout>| {
                    (layout.width().unwrap_or(f32::INFINITY) - width.unwrap_or(f32::INFINITY)).abs()
                };
                gap(one).total_cmp(&gap(other))
            })
    };
    let found = match reuse {
        Reuse::Compatible => exact().or_else(compatible),
        Reuse::Nearest => exact().or_else(compatible).or_else(near),
    }
    .cloned()?;
    drop(shard);
    if found.serves(width, false) {
        front::keep(front_key, &found);
    }
    Some(found)
}

fn front_key(key: u64, width: Option<f32>) -> u64 {
    let width = width.map_or(u64::MAX, |width| u64::from(width.to_bits()));
    key ^ width.wrapping_mul(GOLDEN_GAMMA)
}

fn store(key: u64, shape: &TextShape<'_>, layout: &Arc<TextLayout>) {
    let mut shard = LAYOUT_CACHE.shard(key);
    let bits = layout.width().map(f32::to_bits);
    let mut widths: Layouts = shard.get(key).map_or_else(Vec::new, |widths| {
        matching(widths, shape, layout.request())
            .filter(|cached| cached.width().map(f32::to_bits) != bits)
            .cloned()
            .collect()
    });
    widths.insert(0, Arc::clone(layout));
    widths.truncate(MAX_LAYOUTS_PER_KEY);
    let bytes = widths.iter().map(|layout| layout.bytes()).sum();
    shard.insert(key, widths, bytes);
    drop(shard);
    front::keep(front_key(key, layout.width()), layout);
}

fn layout_with(
    shape: TextShape<'_>,
    width: Option<f32>,
    letter_spacing: f32,
    reuse: Reuse,
) -> Arc<TextLayout> {
    let (key, request) = layout_key(&shape, width, letter_spacing);
    if let Some(found) = lookup(key, &shape, &request, reuse) {
        return found;
    }
    let layout = Arc::new(TextLayout::new(shaped(shape), request));
    store(key, &shape, &layout);
    layout
}
