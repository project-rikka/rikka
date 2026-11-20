use std::f32::consts::TAU;
use std::sync::Arc;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use rikka_types::{Color, Point};
use rikka_ui::{Element, Layout, Length, PaintCtx, View, Widget, motion};
use shell_kit::{Period, WallClock};

use crate::widgets::paint::stroke;

const FINE_MARKS: f32 = 150.0;
const FACE_ALPHA: f32 = 0.05;
const MINOR_ALPHA: f32 = 0.45;

const HOUR: Hand = Hand {
    reach: 0.5,
    tail: 0.08,
    width: 0.04,
};
const MINUTE: Hand = Hand {
    reach: 0.76,
    tail: 0.1,
    width: 0.028,
};
const SECOND: Hand = Hand {
    reach: 0.86,
    tail: 0.2,
    width: 0.01,
};

#[derive(Clone, Copy)]
struct Hand {
    reach: f32,
    tail: f32,
    width: f32,
}

#[derive(Debug)]
pub(crate) struct Dial {
    wall: WallClock,
    zone: Option<Arc<TimeZone>>,
    seconds: bool,
    diameter: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Hands {
    aimed: Option<[f32; 3]>,
}

#[must_use]
pub(super) fn dial(
    wall: WallClock,
    zone: Option<Arc<TimeZone>>,
    seconds: bool,
    diameter: f32,
) -> Dial {
    Dial {
        wall,
        zone,
        seconds,
        diameter,
    }
}

impl Widget for Dial {
    type State = Hands;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::px(self.diameter), Length::px(self.diameter))
            .fixed()
    }

    fn paint(&self, hands: &mut Hands, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let ink = ctx.env().text_color;
        let bounds = ctx.bounds();
        let center = bounds.center();
        let radius = bounds.width().min(bounds.height()) / 2.0;
        ctx.draw_circle(center, radius * 2.0, ink.scale_alpha(FACE_ALPHA));
        self.draw_marks(ctx, center, radius, palette.text_muted);

        let zoned = Timestamp::now().to_zoned(self.zone());
        let second = f32::from(zoned.second());
        let minute = f32::from(zoned.minute())
            + match self.seconds {
                true => second / 60.0,
                false => 0.0,
            };
        let hour = f32::from(zoned.hour() % 12) + minute / 60.0;
        let aimed = [hour / 12.0 * TAU, minute / 60.0 * TAU, second / 60.0 * TAU];
        let before = hands.aimed.replace(aimed);
        let mut turned = [0.0; 3];
        for (index, angle) in aimed.iter().enumerate() {
            if before.is_some_and(|before| before[index] > *angle) {
                ctx.shift(("hand", index), -TAU);
            }
            turned[index] = ctx.animate_curve(("hand", index), *angle, motion::VALUE);
        }

        draw_hand(ctx, center, radius, HOUR, turned[0], ink);
        draw_hand(ctx, center, radius, MINUTE, turned[1], ink);
        if self.seconds {
            draw_hand(ctx, center, radius, SECOND, turned[2], palette.accent);
        }
        ctx.draw_circle(center, radius * 0.1, ink);
        let pin = match self.seconds {
            true => palette.accent,
            false => palette.surface,
        };
        ctx.draw_circle(center, radius * 0.045, pin);
        self.wall.painting(match self.seconds {
            true => Period::Second,
            false => Period::Minute,
        });
    }

    fn same(&self, other: &Self) -> bool {
        let zone = match (&self.zone, &other.zone) {
            (Some(mine), Some(theirs)) => Arc::ptr_eq(mine, theirs) || mine == theirs,
            (None, None) => true,
            _ => false,
        };
        zone && self.seconds == other.seconds && self.diameter == other.diameter
    }
}

impl Dial {
    fn zone(&self) -> TimeZone {
        self.zone
            .as_deref()
            .cloned()
            .unwrap_or_else(TimeZone::system)
    }

    fn draw_marks(&self, ctx: &mut PaintCtx<'_>, center: Point, radius: f32, color: Color) {
        let fine = self.diameter >= FINE_MARKS;
        for mark in 0..60 {
            let hourly = mark % 5 == 0;
            if !hourly && !fine {
                continue;
            }
            let angle = mark as f32 / 60.0 * TAU;
            let (sin, cos) = angle.sin_cos();
            let toward = |reach: f32| Point::new(center.x + sin * reach, center.y - cos * reach);
            match hourly {
                true => {
                    let quarter = mark % 15 == 0;
                    let inner = match quarter {
                        true => 0.8,
                        false => 0.85,
                    };
                    let width = radius
                        * match quarter {
                            true => 0.035,
                            false => 0.025,
                        };
                    stroke(
                        ctx,
                        toward(radius * inner),
                        toward(radius * 0.93),
                        width,
                        color,
                    );
                }
                false => ctx.draw_circle(
                    toward(radius * 0.92),
                    (radius * 0.022).max(1.5),
                    color.scale_alpha(MINOR_ALPHA),
                ),
            }
        }
    }
}

fn draw_hand(
    ctx: &mut PaintCtx<'_>,
    center: Point,
    radius: f32,
    hand: Hand,
    angle: f32,
    color: Color,
) {
    let (sin, cos) = angle.sin_cos();
    let along = |reach: f32| Point::new(center.x + sin * reach, center.y - cos * reach);
    let width = (radius * 2.0 * hand.width).max(1.5);
    stroke(
        ctx,
        along(-radius * hand.tail),
        along(radius * hand.reach),
        width,
        color,
    );
}

impl View for Dial {
    fn build(self) -> Element {
        Element::new(self)
    }
}
