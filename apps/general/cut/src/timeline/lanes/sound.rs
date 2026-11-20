use cut_project::{Clip, Edge, Mix, MixField, Property};
use rikka_app::prelude::*;

use super::parts::{fade_handle, level_keys, level_y};
use crate::timeline::scale::Scale;

const VOLUME: Property = Property::Mix(MixField::Volume);
const LEVEL: f32 = 1.5;
const LEVEL_KEY: f32 = 7.0;
const FADED: f32 = 0.45;
const LINE: f32 = 1.0;

pub(super) struct Sound<'a> {
    pub(super) scale: &'a Scale,
    pub(super) left: f32,
    pub(super) band: Rect,
    pub(super) clip: &'a Clip,
    pub(super) preview: Option<Mix>,
}

impl Sound<'_> {
    pub(super) fn paint(&self, ctx: &mut PaintCtx<'_>) {
        self.paint_fades(ctx);
        self.paint_level(ctx);
    }

    fn frame_at(&self, x: f32) -> f64 {
        self.scale.frame_at(x - self.left)
    }

    fn x_of(&self, frame: i64) -> f32 {
        self.left + self.scale.x_of(frame as f64)
    }

    fn paint_level(&self, ctx: &mut PaintCtx<'_>) {
        let ink = palette().warning;
        let Some(visible) = self.band.intersection(&ctx.visible()) else {
            return;
        };
        let keyed = self.preview.is_none() && self.clip.is_keyed(&VOLUME);
        if !keyed {
            let volume = self
                .clip
                .audible_mix(self.clip.start() as f64, self.preview)
                .map_or(1.0, |mix| mix.volume);
            let y = level_y(self.band, volume);
            ctx.draw_quad(
                Quad::new(Rect::new(
                    visible.x(),
                    y - LEVEL / 2.0,
                    visible.width(),
                    LEVEL,
                ))
                .fill(ink),
            );
            return;
        }
        let level = |x: f32| {
            let volume = self
                .clip
                .audible_mix(self.frame_at(x), None)
                .map_or(1.0, |mix| mix.volume);
            level_y(self.band, volume)
        };
        let mut x = visible.x().floor();
        let mut before = level(x);
        while x < visible.max_x() {
            let here = level(x + 1.0);
            let (top, bottom) = (before.min(here), before.max(here));
            ctx.draw_quad(
                Quad::new(Rect::new(x, top - LEVEL / 2.0, 1.0, bottom - top + LEVEL)).fill(ink),
            );
            before = here;
            x += 1.0;
        }
        let spec = IconGlyph::from(MaterialSymbol::DIAMOND)
            .spec(LEVEL_KEY)
            .fill(1.0);
        for (_, spot) in level_keys(self.scale, self.clip, self.band) {
            let spot = Point::new(spot.x + self.left, spot.y);
            ctx.draw_icon(
                Rect::from_center(spot, Size::new(LEVEL_KEY, LEVEL_KEY)),
                spec,
                ink,
            );
        }
    }

    fn paint_fades(&self, ctx: &mut PaintCtx<'_>) {
        let clip = self.clip;
        let Some(mix) = self
            .preview
            .or_else(|| clip.treatment().map(|treatment| treatment.mix))
        else {
            return;
        };
        let palette = palette();
        let shade = palette.background.scale_alpha(FADED);
        let fades = [
            (
                mix.fade_in,
                clip.start(),
                clip.start() + mix.fade_in.length,
                clip.start(),
            ),
            (
                mix.fade_out,
                clip.end() - mix.fade_out.length,
                clip.end(),
                clip.end(),
            ),
        ];
        for (fade, from, to, origin) in fades {
            if to <= from {
                continue;
            }
            let visible = ctx.visible();
            let mut x = self.x_of(from).max(visible.x()).floor();
            while x < self.x_of(to).min(visible.max_x()) {
                let progress = (self.frame_at(x) - origin as f64).abs() / fade.length as f64;
                let depth = self.band.height() * (1.0 - fade.shape.gain(progress) as f32);
                ctx.draw_quad(Quad::new(Rect::new(x, self.band.y(), 1.0, depth)).fill(shade));
                ctx.draw_quad(
                    Quad::new(Rect::new(x, self.band.y() + depth - LINE / 2.0, 1.0, LINE))
                        .fill(palette.text),
                );
                x += 1.0;
            }
        }
        for edge in [Edge::Head, Edge::Tail] {
            let handle = fade_handle(self.scale, clip, self.band, mix, edge)
                .translate(Point::new(self.left, 0.0));
            ctx.draw_quad(
                Quad::new(handle)
                    .fill(palette.text)
                    .stroke(LINE, palette.background)
                    .radius(Corner::Detail),
            );
        }
    }
}
