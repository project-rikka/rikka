use rikka_types::{Color, Point, RoundedRect};
use rikka_ui::{PaintCtx, Quad, motion};

use crate::controls::press::Press;

const RIPPLE_ALPHA: f32 = 0.10;

const MAX_WAVES: usize = 4;
pub(crate) fn engaged(ctx: &PaintCtx<'_>, interactive: bool, down: bool) -> bool {
    interactive && (down || ctx.hovered())
}

pub(crate) fn state_layer(ctx: &PaintCtx<'_>, interactive: bool, down: bool) -> Option<Color> {
    if !engaged(ctx, interactive, down) {
        return None;
    }
    Some(if down {
        ctx.theme().press_layer()
    } else {
        ctx.theme().hover_layer()
    })
}

#[derive(Debug, Default)]
pub(crate) struct Ripple {
    generation: u32,
    waves: Vec<Wave>,
}

impl Ripple {
    pub(crate) fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        press: &Press,
        surface: RoundedRect,
        interactive: bool,
    ) {
        let down = interactive && press.is_down();
        let target = state_layer(ctx, interactive, down);
        if let Some(fill) = ctx.fade("ripple.layer", target, motion::HOVER) {
            ctx.draw_quad(Quad::new(surface.rect).fill(fill).radius(surface.radii));
        }

        let (generation, origin) = press.last();
        let home = ctx.bounds().origin;
        if self.generation != generation {
            self.generation = generation;
            if self.waves.len() == MAX_WAVES {
                self.waves.remove(0).forget(ctx);
            }
            self.waves
                .push(Wave::new(generation, home + origin, surface, home));
        }
        if !interactive {
            for wave in self.waves.drain(..) {
                wave.forget(ctx);
            }
        }
        if self.waves.is_empty() {
            return;
        }

        let tint = ctx.theme().palette().overlay;
        let mut remaining = self.waves.len();
        self.waves.retain(|wave| {
            remaining -= 1;
            let alive = wave.paint(ctx, surface, tint, down && remaining == 0);
            if !alive {
                wave.forget(ctx);
            }
            alive
        });
    }
}

#[derive(Debug)]
struct Wave {
    id: u32,
    from_home: Point,
    radius: f32,
}

impl Wave {
    fn new(id: u32, origin: Point, surface: RoundedRect, home: Point) -> Self {
        let origin = if surface.rect.contains(origin) {
            origin
        } else {
            surface.rect.center()
        };
        Self {
            id,
            from_home: origin - home,
            radius: reach(origin, surface),
        }
    }

    fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        surface: RoundedRect,
        tint: Color,
        pressed: bool,
    ) -> bool {
        let spread = ctx.play(("ripple.spread", self.id), motion::RIPPLE);
        let holding = pressed || spread < 1.0;
        let alpha = ctx.animate_curve(
            ("ripple.alpha", self.id),
            if holding { 1.0 } else { 0.0 },
            if holding {
                motion::PRESS
            } else {
                motion::RIPPLE_FADE
            },
        );
        if alpha <= 0.0 {
            return false;
        }
        let fill = tint.with_alpha(RIPPLE_ALPHA * alpha);
        let origin = ctx.bounds().origin + self.from_home;
        ctx.clipped(surface, |ctx| {
            ctx.draw_circle(origin, self.radius * 2.0 * spread, fill);
        });
        true
    }

    fn forget(&self, ctx: &mut PaintCtx<'_>) {
        ctx.restart(("ripple.spread", self.id));
        ctx.restart(("ripple.alpha", self.id));
    }
}

fn reach(origin: Point, surface: RoundedRect) -> f32 {
    let rect = surface.rect;
    let radii = surface.radii.clamp_for_size(rect.size);
    let (left, right) = (rect.min_x(), rect.max_x());
    let (top, bottom) = (rect.min_y(), rect.max_y());
    let arc = |x, y, radius: f32| Point::new(x, y).distance(origin) + radius;
    let (tl, tr, br, bl) = (
        radii.top_left,
        radii.top_right,
        radii.bottom_right,
        radii.bottom_left,
    );
    arc(left + tl, top + tl, tl)
        .max(arc(right - tr, top + tr, tr))
        .max(arc(right - br, bottom - br, br))
        .max(arc(left + bl, bottom - bl, bl))
}
