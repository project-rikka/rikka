use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, ImageSource, Layout};

use crate::widgets::{Press, right_click};

pub(crate) fn context_area(body: impl View, open: Handler<(Point,)>) -> Element {
    right_click(
        Layout::column().size(Length::FILL, Length::FILL),
        true,
        body,
        open.map(|(press,): (Press,)| (press.at,)),
    )
}

pub(crate) fn context_row(open_menu: Handler<(Modifiers,)>, body: impl View) -> Element {
    right_click(
        Layout::column(),
        false,
        body,
        open_menu.map(|(press,): (Press,)| (press.modifiers,)),
    )
}

pub(crate) fn tap(body: impl View, press: Handler<(Modifiers,)>) -> Element {
    Element::with_children(Tap { press }, vec![body.build()])
}

struct Tap {
    press: Handler<(Modifiers,)>,
}

impl Widget for Tap {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        if ctx.hovered() {
            let fill = ctx.palette().text.with_alpha(0.08);
            ctx.draw_quad(Quad::new(ctx.bounds()).fill(fill).radius(Corner::Item));
        }
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerEntered | Event::PointerExited => ctx.request_paint(),
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } => ctx.set_handled(),
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                modifiers,
            } if ctx.bounds().contains(position) => {
                ctx.call(&self.press, (modifiers,));
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn cursor(&self, (): &(), _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(CursorShape::Pointer)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Still {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Arc<[u8]>,
    pub(crate) delay: Duration,
}

impl Still {
    pub(crate) fn source(&self) -> ImageSource {
        ImageSource::Rgba {
            width: self.width,
            height: self.height,
            pixels: self.pixels.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Frames {
    pub(crate) shown: Vec<Still>,
    pub(crate) size: Size,
}

impl Frames {
    fn length(&self) -> Duration {
        self.shown.iter().map(|still| still.delay).sum()
    }

    pub(crate) fn paint(&self, started: &mut Option<Instant>, rect: Rect, ctx: &mut PaintCtx<'_>) {
        let now = ctx.now();
        let began = *started.get_or_insert(now);
        let (index, left) = self.at(now.saturating_duration_since(began));
        if let Some(still) = self.shown.get(index) {
            ctx.draw_image(rect, still.source());
        }
        if left != Duration::MAX {
            ctx.wake_at(now + left);
        }
    }

    fn at(&self, elapsed: Duration) -> (usize, Duration) {
        let length = self.length();
        if length.is_zero() || self.shown.len() < 2 {
            return (0, Duration::MAX);
        }
        let mut into = Duration::from_nanos((elapsed.as_nanos() % length.as_nanos()) as u64);
        for (index, still) in self.shown.iter().enumerate() {
            if into < still.delay {
                return (index, still.delay - into);
            }
            into -= still.delay;
        }
        (0, self.shown[0].delay)
    }
}

pub(crate) fn animated(frames: Arc<Frames>, fit: ImageFit) -> Element {
    Element::new(Animated { frames, fit })
}

struct Animated {
    frames: Arc<Frames>,
    fit: ImageFit,
}

impl Widget for Animated {
    type State = Option<Instant>;

    fn layout(&self) -> Layout {
        Layout::DEFAULT
    }

    fn paint(&self, started: &mut Option<Instant>, ctx: &mut PaintCtx<'_>) {
        let rect = fitted(ctx.bounds(), self.frames.size, self.fit);
        self.frames.paint(started, rect, ctx);
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frames, &other.frames) && self.fit == other.fit
    }
}

pub(crate) fn fitted(bounds: Rect, natural: Size, fit: ImageFit) -> Rect {
    if natural.width <= 0.0 || natural.height <= 0.0 {
        return bounds;
    }
    let across = bounds.width() / natural.width;
    let down = bounds.height() / natural.height;
    let scale = match fit {
        ImageFit::Cover => across.max(down),
        _ => across.min(down),
    };
    Rect::from_center(
        bounds.center(),
        Size::new(natural.width * scale, natural.height * scale),
    )
}
