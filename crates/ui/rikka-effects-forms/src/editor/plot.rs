use rikka_image_edit::{BINS, Histogram, HistogramChannel};
use rikka_types::Color;
use rikka_ui::{PaintCtx, Quad, Rect};

const SPIKES: usize = 64;

pub(crate) fn bins(histogram: &Histogram, channel: HistogramChannel, count: usize) -> Vec<f32> {
    let source = histogram.channel(channel);
    let width = (BINS / count.max(1)).max(1);
    let summed: Vec<f32> = source
        .chunks(width)
        .map(|chunk| chunk.iter().map(|&value| value as f32).sum())
        .collect();
    let mut sorted = summed.clone();
    sorted.sort_by(f32::total_cmp);
    let cap = sorted
        .get(sorted.len().saturating_sub(1 + sorted.len() / SPIKES))
        .copied()
        .unwrap_or(0.0)
        .max(1.0);
    summed
        .into_iter()
        .map(|value| (value / cap).min(1.0))
        .collect()
}

pub(crate) fn draw_bins(ctx: &mut PaintCtx<'_>, area: Rect, bins: &[f32], color: Color) {
    if bins.is_empty() {
        return;
    }
    let width = area.width() / bins.len() as f32;
    for (index, value) in bins.iter().enumerate() {
        let height = area.height() * value;
        if height <= 0.0 {
            continue;
        }
        ctx.draw_quad(
            Quad::new(Rect::new(
                area.x() + index as f32 * width,
                area.max_y() - height,
                width.max(1.0),
                height,
            ))
            .fill(color),
        );
    }
}

pub(crate) fn draw_graph(
    ctx: &mut PaintCtx<'_>,
    area: Rect,
    curve: impl Fn(f32) -> f32,
    color: Color,
    thickness: f32,
) {
    let columns = area.width().ceil().max(1.0) as usize;
    let y = |value: f32| area.max_y() - value.clamp(0.0, 1.0) * area.height();
    let mut previous = y(curve(0.0));
    for column in 0..columns {
        let next = y(curve((column + 1) as f32 / columns as f32));
        let (top, bottom) = (previous.min(next), previous.max(next));
        ctx.draw_quad(
            Quad::new(Rect::new(
                area.x() + column as f32,
                top - thickness / 2.0,
                1.0,
                bottom - top + thickness,
            ))
            .fill(color),
        );
        previous = next;
    }
}

pub(crate) fn draw_grid(ctx: &mut PaintCtx<'_>, area: Rect, divisions: usize, color: Color) {
    for step in 1..divisions {
        let along = step as f32 / divisions as f32;
        let x = area.x() + area.width() * along;
        let y = area.y() + area.height() * along;
        ctx.draw_quad(Quad::new(Rect::new(x, area.y(), 1.0, area.height())).fill(color));
        ctx.draw_quad(Quad::new(Rect::new(area.x(), y, area.width(), 1.0)).fill(color));
    }
}

pub(crate) fn draw_ramp(
    ctx: &mut PaintCtx<'_>,
    area: Rect,
    color_at: impl Fn(f32) -> Color,
    slices: usize,
) {
    let width = area.width() / slices as f32;
    for slice in 0..slices {
        let along = (slice as f32 + 0.5) / slices as f32;
        ctx.draw_quad(
            Quad::new(Rect::new(
                area.x() + slice as f32 * width,
                area.y(),
                width + 0.5,
                area.height(),
            ))
            .fill(color_at(along)),
        );
    }
}
