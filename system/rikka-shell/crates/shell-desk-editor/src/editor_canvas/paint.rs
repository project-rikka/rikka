use std::sync::Arc;

use desk_layout::Projection;
use rikka_theme::{Corner, Theme};
use rikka_types::{CornerRadii, Rect, RoundedRect, Size, WindowView};
use rikka_ui::{
    Curve, DurationToken, Easing, IconGlyph, MaterialSymbol, Motion, PaintCtx, Property, Quad,
    is_exiting, motion, theme,
};
use shell_wallpaper::stage::{CARD_RADIUS, RING_WIDTH, THUMB_RADIUS, draw_fitted_image};

use super::frame::Frame;
use super::{EditorCanvas, EditorCanvasState, Hits, Thumb};
use crate::EXIT_DURATION;
use crate::session::Shot;
const SILHOUETTE: f32 = 0.5;
const SILHOUETTE_LEAST: f32 = 1.5;
const WAITING: f32 = 0.34;
const WINDOWS_GONE: f32 = 0.5;

const SPREAD: &str = "spread";
const SCROLL: &str = "scroll";

const ARRIVING: Curve = Motion::enter(DurationToken::Medium2).curve(Property::Spatial);
const DEPARTING: Curve = Motion::exit(EXIT_DURATION).curve(Property::Spatial);
const WALLPAPER_FADE: Curve = Curve::Tween {
    easing: Easing::Standard,
    duration: DurationToken::Medium4,
};
#[must_use]
pub(crate) fn card_radius(spread: f32) -> f32 {
    CARD_RADIUS * spread
}

impl EditorCanvas {
    pub(super) fn draw(&self, state: &mut EditorCanvasState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        let sheet = &self.sheet;
        let theme = theme().clone();
        let spread = ctx
            .progress(SPREAD, !is_exiting(), ARRIVING, DEPARTING)
            .clamp(0.0, 1.0);
        sheet.frost.set(&sheet.output, spread);
        let goal = match is_exiting() {
            true => 0.0,
            false => 1.0,
        };
        let frame = Frame::new(bounds, sheet.taskbar, sheet.thumbs.len());
        let card = frame.card(spread);
        let projection = Projection::new(bounds, card);
        sheet.session.frame(Shot {
            card,
            projection,
            spread,
            moving: (spread - goal).abs() > f32::EPSILON,
        });
        state.settled = spread >= 1.0 && !is_exiting();
        let scroll = self.scroll_strip(state, &frame, ctx);
        state.hits = Hits {
            card,
            projection: Some(projection),
            thumbs: Vec::new(),
            strip: frame.strip(),
            overflow: frame.overflow(),
        };
        ctx.clipped(frame.stage, |ctx| {
            self.draw_card(card, spread, &theme, ctx);
            self.draw_behind(projection, card, spread, ctx);
            self.draw_thumbs(state, &frame, spread, scroll, &theme, ctx);
        });
    }

    fn scroll_strip(
        &self,
        state: &mut EditorCanvasState,
        frame: &Frame,
        ctx: &mut PaintCtx<'_>,
    ) -> f32 {
        let current = self.sheet.current;
        if state.last_drawn != current {
            let first = state.last_drawn.is_none();
            state.last_drawn = current;
            if let Some(index) = current {
                state.scroll = frame.showing(index, state.scroll);
            }
            if first {
                ctx.jump(SCROLL, state.scroll);
            }
        }
        state.scroll = state.scroll.clamp(0.0, frame.overflow());
        ctx.animate_curve(SCROLL, state.scroll, motion::SCROLL)
    }

    fn draw_behind(&self, projection: Projection, card: Rect, spread: f32, ctx: &mut PaintCtx<'_>) {
        let shown = 1.0 - spread / WINDOWS_GONE;
        if shown <= 0.0 {
            return;
        }
        let shape = RoundedRect::new(card, CornerRadii::uniform(card_radius(spread)));
        for (id, real) in &self.sheet.behind {
            ctx.draw_window(
                WindowView::new(Arc::clone(id), projection.rect(*real))
                    .clip(shape)
                    .opacity(shown),
            );
        }
    }

    fn draw_card(&self, card: Rect, spread: f32, theme: &Theme, ctx: &mut PaintCtx<'_>) {
        let pictures = &self.sheet.pictures;
        let shape = RoundedRect::new(card, CornerRadii::uniform(card_radius(spread)));
        ctx.clipped(shape, |ctx| {
            if pictures.under.is_none() && pictures.over.is_none() {
                ctx.draw_quad(Quad::new(card).fill(theme.palette().background));
            }
            if let Some(under) = &pictures.under {
                draw_fitted_image(ctx, card, under);
            }
            if let Some(over) = &pictures.over {
                let shown = match over.fades {
                    true => ctx.play(("wear", over.serial), WALLPAPER_FADE),
                    false => 1.0,
                };
                ctx.with_opacity(shown, |ctx| draw_fitted_image(ctx, card, &over.source));
            }
        });
    }

    fn draw_thumbs(
        &self,
        state: &mut EditorCanvasState,
        frame: &Frame,
        spread: f32,
        scroll: f32,
        theme: &Theme,
        ctx: &mut PaintCtx<'_>,
    ) {
        if spread <= 0.0 || self.sheet.thumbs.is_empty() {
            return;
        }
        let strip = frame.strip();
        let shown = Rect::new(
            strip.min_x() - RING_WIDTH * 2.0,
            frame.stage.rect.min_y(),
            strip.width() + RING_WIDTH * 4.0,
            strip.max_y() + RING_WIDTH * 2.0 - frame.stage.rect.min_y(),
        );
        let output = frame.output();
        ctx.clipped(shown, |ctx| {
            ctx.with_opacity(spread, |ctx| {
                for (index, thumb) in self.sheet.thumbs.iter().enumerate() {
                    let rect = frame.thumb(index, spread, scroll);
                    if !rect.intersects(&shown) {
                        continue;
                    }
                    let current = self.sheet.current == Some(index);
                    let hovered = state.thumb == Some(index);
                    draw_thumb(thumb, rect, output, current, hovered, theme, ctx);
                    state.hits.thumbs.push((index, rect));
                }
            });
        });
    }
}

fn draw_thumb(
    thumb: &Thumb,
    rect: Rect,
    output: Rect,
    current: bool,
    hovered: bool,
    theme: &Theme,
    ctx: &mut PaintCtx<'_>,
) {
    let palette = theme.palette();
    let shape = RoundedRect::new(rect, CornerRadii::uniform(THUMB_RADIUS));
    ctx.draw_quad(
        Quad::new(rect)
            .radius(THUMB_RADIUS)
            .fill(palette.surface_variant),
    );
    ctx.clipped(shape, |ctx| {
        if thumb
            .preview
            .as_ref()
            .is_none_or(|source| source.natural_size().is_none())
        {
            waiting(rect, thumb.motion, theme, ctx);
        }
        if let Some(source) = &thumb.preview {
            let fitted =
                shell_wallpaper::FIT.place(rect, source.natural_size().unwrap_or(Size::ZERO));
            ctx.draw_image(fitted, source.clone());
        }
        let projection = Projection::new(output, rect);
        let radius = (Corner::Window.radius() * projection.scale()).max(SILHOUETTE_LEAST);
        for silhouette in &thumb.silhouettes {
            let placed = projection.rect(*silhouette);
            ctx.draw_quad(
                Quad::new(placed)
                    .radius(radius)
                    .fill(palette.surface.with_alpha(SILHOUETTE))
                    .stroke(SILHOUETTE, palette.text.with_alpha(WAITING)),
            );
        }
    });
    let edge = match (current, hovered) {
        (true, _) => Some(palette.accent),
        (false, true) => Some(palette.text_muted),
        _ => None,
    };
    if let Some(edge) = edge {
        ctx.draw_quad(
            Quad::new(rect.inflate(RING_WIDTH))
                .radius(THUMB_RADIUS + RING_WIDTH)
                .stroke(RING_WIDTH, edge),
        );
    }
}

fn waiting(rect: Rect, motion: bool, theme: &Theme, ctx: &mut PaintCtx<'_>) {
    let symbol = match motion {
        true => MaterialSymbol::MOVIE,
        false => MaterialSymbol::IMAGE,
    };
    let size = (rect.height() * WAITING).min(24.0);
    ctx.draw_icon(
        Rect::from_center(rect.center(), Size::new(size, size)),
        IconGlyph::from(symbol).spec(size),
        theme.palette().text_muted,
    );
}
