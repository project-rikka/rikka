use std::hash::Hash;

use rikka_animation::Curve;
use rikka_types::Color;

use crate::motion;
use crate::paint::PaintCtx;

const AT: &str = "at";

const FILL: &str = "fill";

const SPAN: &str = "span";

#[derive(Debug, Default, PartialEq)]
enum Arrival {
    #[default]
    Fresh,
    Rides(f32),
    Travels,
}

fn arrival(seat: Option<(usize, f32)>, slot: usize, at: f32) -> Arrival {
    match seat {
        Some((last, was)) if last == slot => Arrival::Rides(at - was),
        Some(_) => Arrival::Travels,
        None => Arrival::Fresh,
    }
}

#[derive(Debug, Default)]
pub struct Indicator {
    seat: Option<(usize, f32)>,
    arrival: Arrival,
}

impl Indicator {
    pub fn resolve(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        key: impl Hash + Copy,
        place: Option<(usize, f32)>,
        color: Color,
        travel: Curve,
    ) -> Option<(f32, Color)> {
        let shown = match place {
            Some((slot, to)) => {
                self.arrival = arrival(self.seat, slot, to);
                match self.arrival {
                    Arrival::Fresh => ctx.jump((key, AT), to),
                    Arrival::Rides(delta) => ctx.shift((key, AT), delta),
                    Arrival::Travels => {}
                }
                self.seat = Some((slot, to));
                let at = ctx.animate_curve((key, AT), to, travel);
                ctx.fade((key, FILL), Some(color), motion::HOVER)
                    .map(|fill| (at, fill))
            }
            None => {
                let at = ctx.hold((key, AT), None::<f32>, travel);
                ctx.fade((key, FILL), None, motion::HOVER)
                    .zip(at)
                    .map(|(fill, at)| (at, fill))
            }
        };
        if shown.is_none() {
            self.seat = None;
        }
        shown
    }

    pub fn vanish(&mut self, ctx: &mut PaintCtx<'_>, key: impl Hash + Copy) {
        ctx.restart((key, AT));
        ctx.restart((key, FILL));
        ctx.restart((key, SPAN));
        *self = Indicator::default();
    }

    pub fn extent(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        key: impl Hash + Copy,
        reach: Option<f32>,
        travel: Curve,
    ) -> f32 {
        let Some(reach) = reach else {
            return ctx
                .hold((key, SPAN), None::<f32>, travel)
                .unwrap_or_default();
        };
        if self.arrival != Arrival::Travels {
            ctx.jump((key, SPAN), reach);
        }
        ctx.animate_curve((key, SPAN), reach, travel)
    }
}

pub fn emphasis(at: f32, place: f32, span: f32) -> f32 {
    if span <= 0.0 {
        return 0.0;
    }
    1.0 - ((at - place).abs() / span).min(1.0)
}
