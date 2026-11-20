use std::time::{Duration, Instant};

use rikka_sound::Spectrum;
use rikka_types::{Point, PointerButton, Rect};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, PaintCtx, Quad, Style, Widget,
};
use shell_feeds::{Levels, same_spectrum};

const BAR_SHARE: f32 = 0.5;
const MIN_BAR: f32 = 2.0;
const RESTING: f32 = 0.3;
const RESTING_BARS: f32 = 2.4;
const AHEAD_ALPHA: f32 = 0.22;
const HEAD: f32 = 2.0;
const CAUGHT_UP: f32 = 0.01;
const SEEK_PATIENCE: Duration = Duration::from_secs(2);

#[derive(Widget)]
pub(crate) struct Scrubber {
    spectrum: Option<Spectrum>,
    bars: usize,
    #[debug]
    at: Option<f32>,
    on_seek: Action<(f32,)>,
    #[style]
    style: Style,
}

#[derive(Debug, Default)]
pub(crate) struct Scrubbing {
    levels: Levels,
    dragged: Option<f32>,
    sought: Option<PendingSeek>,
}

#[derive(Debug)]
struct PendingSeek {
    to: f32,
    until: Option<Instant>,
}

impl Scrubbing {
    fn shown(&mut self, at: Option<f32>, now: Instant) -> Option<f32> {
        if let Some(sought) = &mut self.sought {
            let until = *sought.until.get_or_insert(now + SEEK_PATIENCE);
            let caught = at.is_some_and(|at| (at - sought.to).abs() < CAUGHT_UP);
            if caught || now >= until {
                self.sought = None;
            }
        }
        self.dragged
            .or(self.sought.as_ref().map(|sought| sought.to))
            .or(at)
    }
}

#[must_use]
pub(super) fn scrubber(spectrum: Option<Spectrum>, bars: usize, at: Option<f32>) -> Scrubber {
    Scrubber {
        spectrum,
        bars: bars.max(2),
        at: at.map(|at| at.clamp(0.0, 1.0)),
        on_seek: Action::default(),
        style: Style::default(),
    }
}

impl Scrubber {
    #[must_use]
    pub(super) fn on_seek<Marker>(mut self, seek: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_seek = Action::new(seek);
        self
    }

    fn seekable(&self) -> bool {
        self.at.is_some() && self.on_seek.is_set()
    }
}

fn fraction_at(bounds: Rect, point: Point) -> f32 {
    match bounds.width() > 0.0 {
        true => ((point.x - bounds.x()) / bounds.width()).clamp(0.0, 1.0),
        false => 0.0,
    }
}

impl Widget for Scrubber {
    type State = Scrubbing;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, state: &mut Scrubbing, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }
        let palette = ctx.palette();
        let ahead = ctx.env().text_color.scale_alpha(AHEAD_ALPHA);
        let pitch = bounds.width() / self.bars as f32;
        let bar = (pitch * BAR_SHARE).max(MIN_BAR).min(bounds.height());
        let resting = (bounds.height() * RESTING)
            .max(bar * RESTING_BARS)
            .min(bounds.height());
        let room = bounds.height() - resting;
        let target = state.shown(self.at, ctx.now());
        let at = target.map(|target| ctx.steer("at", target, state.dragged.is_some()));
        if let Some(until) = state.sought.as_ref().and_then(|sought| sought.until) {
            ctx.wake_at(until);
        }
        let shown = state.levels.listen(self.spectrum.as_ref(), self.bars, ctx);
        for (index, level) in shown.iter().enumerate() {
            let tall = resting + room * level;
            let middle = (index as f32 + 0.5) / self.bars as f32;
            let fill = match at {
                Some(at) if middle > at => ahead,
                _ => palette.accent,
            };
            ctx.draw_quad(
                Quad::new(Rect::new(
                    bounds.x() + pitch * index as f32 + (pitch - bar) / 2.0,
                    bounds.y() + (bounds.height() - tall) / 2.0,
                    bar,
                    tall,
                ))
                .fill(fill)
                .radius(bar / 2.0),
            );
        }
        let active = state.dragged.is_some() || (ctx.hovered() && self.seekable());
        let head = ctx.animate_curve("head", f32::from(u8::from(active)), rikka_ui::motion::VALUE);
        if let Some(at) = at
            && head > 0.0
        {
            let x = bounds.x() + bounds.width() * at - HEAD / 2.0;
            ctx.draw_quad(
                Quad::new(Rect::new(x, bounds.y(), HEAD, bounds.height()))
                    .fill(palette.accent.scale_alpha(head))
                    .radius(HEAD / 2.0),
            );
        }
    }

    fn event(&self, state: &mut Scrubbing, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.seekable() {
            if state.dragged.take().is_some() {
                ctx.release_pointer();
                ctx.request_paint();
            }
            return;
        }
        let bounds = ctx.bounds();
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                state.dragged = Some(fraction_at(bounds, *position));
                ctx.capture_pointer();
                ctx.set_handled();
                ctx.request_paint();
            }
            Event::PointerMoved(position) if state.dragged.is_some() => {
                state.dragged = Some(fraction_at(bounds, *position));
                ctx.set_handled();
                ctx.request_paint();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                position,
                ..
            } if state.dragged.is_some() => {
                state.dragged = None;
                let to = fraction_at(bounds, *position);
                state.sought = Some(PendingSeek { to, until: None });
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                ctx.act(&self.on_seek, (to,));
            }
            Event::PointerEntered | Event::PointerExited => ctx.request_paint(),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Scrubbing) -> Behavior {
        Behavior::TRACKS_HOVER.when(self.seekable())
    }

    fn same(&self, other: &Self) -> bool {
        same_spectrum(self.spectrum.as_ref(), other.spectrum.as_ref())
            && self.bars == other.bars
            && self.at == other.at
            && self.on_seek == other.on_seek
            && self.style == other.style
    }
}
