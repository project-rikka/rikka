use std::collections::BTreeSet;

use cut_project::{Clip, KeyRef};
use rikka_app::prelude::*;
use rikka_figure_editing::paint_line;

use super::points::{self, KeyPoint};
use super::{Curve, Plotting, hue, sample};
use crate::cut::Plot;
use crate::timeline::scale::Scale;

const DIVISIONS: usize = 4;
const GRID: f32 = 0.5;
const ZERO: f32 = 0.9;
const THICKNESS: f32 = 1.5;
const KEY: f32 = 7.0;
const KEY_LINE: f32 = 1.5;
const TIP: f32 = 6.0;

pub(crate) struct Drawing<'a> {
    pub(crate) plotting: &'a Plotting,
    pub(crate) clip: &'a Clip,
    pub(crate) picked: &'a BTreeSet<KeyRef>,
    pub(crate) scale: &'a Scale,
    pub(crate) origin: Point,
    pub(crate) width: f32,
}

impl Drawing<'_> {
    fn placed(&self, at: Point) -> Point {
        Point::new(self.origin.x + at.x, self.origin.y + at.y)
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>) {
        let area = self.plotting.area;
        let plot_area = Rect::new(
            self.origin.x + area.x(),
            self.origin.y + area.y(),
            area.width(),
            area.height(),
        );
        ctx.clipped(plot_area, |ctx| {
            self.paint_grid(ctx, plot_area);
            for curve in &self.plotting.curves {
                self.paint_curve(ctx, curve);
            }
            self.paint_handles(ctx);
            for point in points::keys(self.plotting, self.clip, self.scale) {
                self.paint_key(ctx, &point);
            }
        });
    }

    fn paint_grid(&self, ctx: &mut PaintCtx<'_>, plot_area: Rect) {
        let outline = palette().outline;
        for step in 1..DIVISIONS {
            let y = plot_area.y() + plot_area.height() * step as f32 / DIVISIONS as f32;
            ctx.draw_quad(
                Quad::new(Rect::new(plot_area.x(), y, plot_area.width(), 1.0))
                    .fill(outline.scale_alpha(GRID)),
            );
        }
        if self.plotting.plot != Plot::Speed {
            return;
        }
        for curve in &self.plotting.curves {
            let y = self.origin.y + self.plotting.y_of(curve, 0.0);
            if y > plot_area.y() && y < plot_area.max_y() {
                ctx.draw_quad(
                    Quad::new(Rect::new(plot_area.x(), y, plot_area.width(), 1.0))
                        .fill(outline.scale_alpha(ZERO)),
                );
            }
        }
    }

    fn paint_curve(&self, ctx: &mut PaintCtx<'_>, curve: &Curve) {
        let Some(channel) = self.clip.channel(&curve.property) else {
            return;
        };
        let color = hue(curve.index);
        let last = (self.clip.length() - 1).max(0) as f64;
        let start = self.clip.start() as f64;
        let from = self.scale.x_of(start).max(0.0).floor();
        let to = self.scale.x_of(start + last).min(self.width);
        let mut previous: Option<f32> = None;
        let mut x = from;
        while x <= to {
            let local = (self.scale.frame_at(x) - start).clamp(0.0, last);
            let Some(value) = sample(&channel, self.plotting.plot, local) else {
                x += 1.0;
                continue;
            };
            let y = self.plotting.y_of(curve, value);
            let (top, bottom) =
                previous.map_or((y, y), |previous_y| (previous_y.min(y), previous_y.max(y)));
            ctx.draw_quad(
                Quad::new(Rect::new(
                    self.origin.x + x,
                    self.origin.y + top - THICKNESS / 2.0,
                    1.0,
                    bottom - top + THICKNESS,
                ))
                .fill(color),
            );
            previous = Some(y);
            x += 1.0;
        }
    }

    fn paint_handles(&self, ctx: &mut PaintCtx<'_>) {
        for point in points::handles(self.plotting, self.clip, self.picked, self.scale) {
            let color = hue(point.index);
            let (key, tip) = (self.placed(point.key), self.placed(point.tip));
            paint_line(ctx, key, tip, color);
            ctx.draw_quad(
                Quad::new(Rect::from_center(tip, Size::new(TIP, TIP)))
                    .fill(color)
                    .radius(TIP / 2.0),
            );
        }
    }

    fn paint_key(&self, ctx: &mut PaintCtx<'_>, point: &KeyPoint) {
        let palette = palette();
        let picked = self
            .picked
            .contains(&KeyRef::new(point.property.clone(), point.frame));
        let (fill, line) = match picked {
            true => (palette.accent, palette.accent),
            false => (palette.surface, hue(point.index)),
        };
        ctx.draw_quad(
            Quad::new(Rect::from_center(
                self.placed(point.at),
                Size::new(KEY, KEY),
            ))
            .fill(fill)
            .stroke(KEY_LINE, line),
        );
    }
}
