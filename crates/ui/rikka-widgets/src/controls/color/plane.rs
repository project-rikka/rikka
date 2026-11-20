use rikka_types::{Color, Key, Modifiers, Point, PointerButton, Rect, RoundedRect};
use rikka_ui::{Action, Behavior, Corner, Event, EventCtx, Layout, Length, PaintCtx, Quad, Widget};

use crate::controls::color::{Hsv, paint_checker, textures};
use crate::controls::grab::{Grab, GrabStep};

pub(crate) const SQUARE: f32 = 168.0;
pub(crate) const STRIP: f32 = 14.0;
pub(crate) const GAP: f32 = 10.0;
const RADIUS: f32 = Corner::Item.radius();
const KNOB: f32 = 14.0;
const KNOB_RING: f32 = 2.0;
const NUDGE: f32 = 0.01;
const LARGE_NUDGE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Zone {
    Square,
    Hue,
    Alpha,
}

#[derive(Widget)]
pub(crate) struct Plane {
    #[debug]
    pub(crate) hsv: Hsv,
    pub(crate) alpha: bool,
    pub(crate) on_pick: Action<(Hsv, bool)>,
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub(crate) struct PlaneState {
    grab: Grab<Zone>,
    keyed: Option<Zone>,
    ringed: bool,
}

impl Plane {
    pub(crate) fn height(alpha: bool) -> f32 {
        let strips = match alpha {
            true => 2.0,
            false => 1.0,
        };
        SQUARE + strips * (GAP + STRIP)
    }

    fn zones(&self, bounds: Rect) -> [(Zone, Rect); 3] {
        let width = bounds.width();
        let square = Rect::new(bounds.x(), bounds.y(), width, SQUARE);
        let hue = Rect::new(bounds.x(), square.max_y() + GAP, width, STRIP);
        let alpha = match self.alpha {
            true => Rect::new(bounds.x(), hue.max_y() + GAP, width, STRIP),
            false => Rect::new(bounds.x(), hue.max_y(), width, 0.0),
        };
        [
            (Zone::Square, square),
            (Zone::Hue, hue),
            (Zone::Alpha, alpha),
        ]
    }

    fn zone_at(&self, bounds: Rect, at: Point) -> Option<Zone> {
        self.zones(bounds)
            .into_iter()
            .find(|(_, rect)| rect.height() > 0.0 && reach(*rect).contains(at))
            .map(|(zone, _)| zone)
    }

    fn rect_of(&self, bounds: Rect, zone: Zone) -> Rect {
        self.zones(bounds)
            .into_iter()
            .find(|(cached_zone, _)| *cached_zone == zone)
            .map_or(bounds, |(_, rect)| rect)
    }

    pub(crate) fn picked(&self, bounds: Rect, zone: Zone, at: Point) -> Hsv {
        let rect = self.rect_of(bounds, zone);
        let across = fraction(at.x - rect.x(), rect.width());
        let down = fraction(at.y - rect.y(), rect.height());
        match zone {
            Zone::Square => Hsv {
                saturation: across,
                value: 1.0 - down,
                ..self.hsv
            },
            Zone::Hue => Hsv {
                hue: across,
                ..self.hsv
            },
            Zone::Alpha => Hsv {
                alpha: across,
                ..self.hsv
            },
        }
    }

    fn nudged(&self, zone: Zone, key: Key, modifiers: Modifiers) -> Option<Hsv> {
        let by = match modifiers.shift {
            true => LARGE_NUDGE,
            false => NUDGE,
        };
        let (across, down) = match key {
            Key::ArrowLeft => (-by, 0.0),
            Key::ArrowRight => (by, 0.0),
            Key::ArrowUp => (0.0, by),
            Key::ArrowDown => (0.0, -by),
            _ => return None,
        };
        let hsv = self.hsv;
        let moved = |value: f32, by: f32| (value + by).clamp(0.0, 1.0);
        Some(match zone {
            Zone::Square => Hsv {
                saturation: moved(hsv.saturation, across),
                value: moved(hsv.value, down),
                ..hsv
            },
            Zone::Hue => Hsv {
                hue: moved(hsv.hue, across + down),
                ..hsv
            },
            Zone::Alpha => Hsv {
                alpha: moved(hsv.alpha, across + down),
                ..hsv
            },
        })
    }

    fn pick(&self, hsv: Hsv, done: bool, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.on_pick, (hsv, done));
    }

    fn paint_square(&self, ctx: &mut PaintCtx<'_>, rect: Rect) {
        let shape = RoundedRect::new(rect, RADIUS);
        let ramps = textures();
        ctx.clipped(shape, |ctx| {
            ctx.draw_quad(Quad::new(rect).fill(self.hsv.pure_hue()));
            ctx.draw_mask(rect, ramps.fading_right, Color::WHITE);
            ctx.draw_mask(rect, ramps.rising_down, Color::BLACK);
        });
        let center = Point::new(
            rect.x() + rect.width() * self.hsv.saturation,
            rect.y() + rect.height() * (1.0 - self.hsv.value),
        );
        knob(ctx, center, self.hsv.with_alpha(1.0));
    }

    fn paint_hue(&self, ctx: &mut PaintCtx<'_>, rect: Rect) {
        let shape = RoundedRect::new(rect, rect.height() / 2.0);
        let hues = textures().hues;
        ctx.clipped(shape, |ctx| ctx.draw_image(rect, hues));
        let center = Point::new(rect.x() + rect.width() * self.hsv.hue, rect.center().y);
        knob(ctx, center, self.hsv.pure_hue());
    }

    fn paint_alpha(&self, ctx: &mut PaintCtx<'_>, rect: Rect) {
        let shape = RoundedRect::new(rect, rect.height() / 2.0);
        let opaque = self.hsv.with_alpha(1.0);
        let rising = textures().rising_right;
        paint_checker(ctx, shape);
        ctx.clipped(shape, |ctx| {
            ctx.draw_mask(rect, rising, opaque);
        });
        let center = Point::new(rect.x() + rect.width() * self.hsv.alpha, rect.center().y);
        knob(ctx, center, self.hsv.color());
    }
}

impl Hsv {
    fn with_alpha(self, alpha: f32) -> Color {
        Hsv { alpha, ..self }.color()
    }
}

fn reach(rect: Rect) -> Rect {
    Rect::new(
        rect.x(),
        rect.y() - GAP / 2.0,
        rect.width(),
        rect.height() + GAP,
    )
}

fn fraction(offset: f32, extent: f32) -> f32 {
    match extent > 0.0 {
        true => (offset / extent).clamp(0.0, 1.0),
        false => 0.0,
    }
}

fn knob(ctx: &mut PaintCtx<'_>, center: Point, fill: Color) {
    let rect = Rect::from_circle(center, KNOB / 2.0);
    ctx.draw_quad(
        Quad::new(rect)
            .fill(Color::WHITE)
            .stroke(1.0, Color::rgba(0.0, 0.0, 0.0, 0.45))
            .radius(KNOB / 2.0),
    );
    let inner = rect.inflate(-KNOB_RING);
    if fill.a < 1.0 {
        paint_checker(ctx, RoundedRect::new(inner, inner.width() / 2.0));
    }
    ctx.draw_quad(Quad::new(inner).fill(fill).radius(inner.width() / 2.0));
}

impl Widget for Plane {
    type State = PlaneState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(Plane::height(self.alpha)))
            .fixed()
    }

    fn behavior(&self, _state: &PlaneState) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.on_pick.is_set())
    }

    fn paint(&self, state: &mut PlaneState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let [(_, square), (_, hue), (_, alpha)] = self.zones(bounds);
        self.paint_square(ctx, square);
        self.paint_hue(ctx, hue);
        if self.alpha {
            self.paint_alpha(ctx, alpha);
        }
        if ctx.focused()
            && state.ringed
            && let Some(zone) = state.keyed
        {
            let rect = self.rect_of(bounds, zone).inflate(2.0);
            let accent = ctx.theme().palette().accent;
            let radius = match zone {
                Zone::Square => RADIUS + 2.0,
                Zone::Hue | Zone::Alpha => rect.height() / 2.0,
            };
            ctx.draw_quad(Quad::new(rect).stroke(2.0, accent).radius(radius));
        }
    }

    fn event(&self, state: &mut PlaneState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.on_pick.is_set() {
            return;
        }
        let bounds = ctx.bounds();
        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { held, to }) => {
                return self.pick(self.picked(bounds, held, to), false, ctx);
            }
            Some(GrabStep::Ended { held, at }) => {
                return self.pick(self.picked(bounds, held, at), true, ctx);
            }
            Some(GrabStep::Cancelled { .. }) => return self.pick(self.hsv, true, ctx),
            None => {}
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                let Some(zone) = self.zone_at(bounds, position) else {
                    return;
                };
                state.grab.begin(zone, ctx);
                ctx.request_focus();
                state.keyed = Some(zone);
                state.ringed = false;
                self.pick(self.picked(bounds, zone, position), false, ctx);
            }
            Event::FocusGained => {
                state.ringed = !state.grab.is_held();
                ctx.request_paint();
            }
            Event::FocusLost => state.ringed = false,
            Event::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                let zone = state.keyed.unwrap_or(Zone::Square);
                if let Some(hsv) = self.nudged(zone, key, modifiers) {
                    ctx.set_handled();
                    state.keyed = Some(zone);
                    if !state.ringed {
                        state.ringed = true;
                        ctx.request_paint();
                    }
                    if hsv != self.hsv {
                        self.pick(hsv, true, ctx);
                    }
                }
            }
            _ => {}
        }
    }
}
