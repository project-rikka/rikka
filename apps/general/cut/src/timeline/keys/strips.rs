use std::collections::BTreeSet;

use cut_project::{Clip, KeyRef};
use rikka_app::prelude::*;

use super::marks::{self, Mark};
use super::outline::{LANE, Lane, Outline, SUMMARY};
use crate::timeline::scale::Scale;

const GLYPH: f32 = 11.0;
const SHADE: f32 = 0.07;
const OUTSIDE: f32 = 0.5;
const PARTLY: f32 = 0.55;
const LINE: f32 = 1.0;

pub(crate) struct Strips<'a> {
    pub(crate) clip: &'a Clip,
    pub(crate) outline: &'a Outline,
    pub(crate) picked: &'a BTreeSet<KeyRef>,
    pub(crate) scale: &'a Scale,
    pub(crate) bounds: Rect,
}

impl Strips<'_> {
    pub(crate) fn paint_backdrop(&self, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = self.bounds;
        ctx.draw_quad(Quad::new(bounds).fill(palette.background));
        ctx.draw_quad(Quad::new(bounds).fill(palette.text.scale_alpha(SHADE)));
        let start = bounds.x() + self.scale.x_of(self.clip.start() as f64);
        let end = bounds.x() + self.scale.x_of(self.clip.end() as f64);
        let dim = palette.background.scale_alpha(OUTSIDE);
        if start > bounds.x() {
            ctx.draw_quad(
                Quad::new(Rect::new(
                    bounds.x(),
                    bounds.y(),
                    start - bounds.x(),
                    bounds.height(),
                ))
                .fill(dim),
            );
        }
        if end < bounds.max_x() {
            ctx.draw_quad(
                Quad::new(Rect::new(
                    end,
                    bounds.y(),
                    bounds.max_x() - end,
                    bounds.height(),
                ))
                .fill(dim),
            );
        }
        let lines = match self.outline.graph {
            true => 0,
            false => self.outline.properties.len(),
        };
        for index in 0..=lines {
            self.paint_line(ctx, Outline::lane_top(index));
        }
    }

    fn paint_line(&self, ctx: &mut PaintCtx<'_>, top: f32) {
        ctx.draw_quad(
            Quad::new(Rect::new(
                self.bounds.x(),
                self.bounds.y() + top - LINE,
                self.bounds.width(),
                LINE,
            ))
            .fill(palette().outline),
        );
    }

    pub(crate) fn paint_marks(&self, ctx: &mut PaintCtx<'_>) {
        self.paint_lane(ctx, Lane::Summary, SUMMARY / 2.0);
        if self.outline.graph {
            return;
        }
        for index in 0..self.outline.properties.len() {
            self.paint_lane(
                ctx,
                Lane::Property(index),
                Outline::lane_top(index) + LANE / 2.0,
            );
        }
    }

    fn paint_lane(&self, ctx: &mut PaintCtx<'_>, lane: Lane, middle: f32) {
        let palette = palette();
        let left = self.scale.first();
        let right = self.scale.frame_at(self.bounds.width());
        for mark in marks::in_lane(self.clip, self.outline, lane) {
            if (mark.frame as f64) < left - 1.0 || (mark.frame as f64) > right + 1.0 {
                continue;
            }
            let color = match self.share(&mark) {
                Share::All => palette.accent,
                Share::Some => palette.accent.scale_alpha(PARTLY),
                Share::None => palette.text,
            };
            let center = Point::new(
                self.bounds.x() + self.scale.x_of(mark.frame as f64),
                self.bounds.y() + middle,
            );
            mark.glyph.paint(ctx, center, GLYPH, color);
        }
    }

    fn share(&self, mark: &Mark) -> Share {
        let mark_kind = mark
            .keys
            .iter()
            .filter(|key| self.picked.contains(*key))
            .count();
        match mark_kind {
            0 => Share::None,
            all if all == mark.keys.len() => Share::All,
            _ => Share::Some,
        }
    }
}

enum Share {
    All,
    Some,
    None,
}
