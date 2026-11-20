use std::time::{Duration, Instant};

use rikka_sound::Spectrum;
use rikka_types::{Color, Rect};
use rikka_ui::{Element, Layout, Length, PaintCtx, Quad, View, Widget};

const RISE: Duration = Duration::from_millis(40);
const FALL: Duration = Duration::from_millis(180);
const SETTLED: f32 = 0.004;
const LISTEN: Duration = Duration::from_millis(120);
const HEARD: Duration = Duration::from_millis(16);
const BAR_SHARE: f32 = 0.6;

pub struct Waveform {
    spectrum: Option<Spectrum>,
    bars: usize,
    height: f32,
    bar: f32,
    color: Color,
}

#[derive(Debug, Default)]
pub struct Levels {
    event: Vec<f32>,
    shown: Vec<f32>,
    painted: Option<Instant>,
}

pub fn waveform(spectrum: Option<Spectrum>, bars: usize, height: f32) -> Waveform {
    Waveform {
        spectrum,
        bars: bars.max(1),
        height,
        bar: (height / 5.0).max(2.0),
        color: Color::TRANSPARENT,
    }
}

impl Waveform {
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    fn width(&self) -> f32 {
        let pitch = self.bar / BAR_SHARE;
        pitch * (self.bars - 1) as f32 + self.bar
    }
}

impl Levels {
    pub fn listen(
        &mut self,
        spectrum: Option<&Spectrum>,
        bars: usize,
        ctx: &mut PaintCtx<'_>,
    ) -> &[f32] {
        self.event.resize(bars, 0.0);
        self.shown.resize(bars, 0.0);
        let now = ctx.now();
        let elapsed = self
            .painted
            .replace(now)
            .map_or(Duration::ZERO, |then| now.saturating_duration_since(then));
        let fresh = spectrum.is_some_and(|spectrum| spectrum.read(&mut self.event));
        if !fresh {
            self.event.fill(0.0);
        }
        let mut moving = false;
        for (shown, event) in self.shown.iter_mut().zip(&self.event) {
            *shown = follow(*shown, *event, elapsed);
            moving |= (*shown - *event).abs() > SETTLED;
        }
        if moving {
            ctx.request_frame();
        } else if spectrum.is_some() {
            ctx.wake_at(now + if fresh { HEARD } else { LISTEN });
        }
        &self.shown
    }
}

impl Widget for Waveform {
    type State = Levels;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::px(self.width()), Length::px(self.height))
            .fixed()
    }

    fn paint(&self, levels: &mut Levels, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let pitch = self.bar / BAR_SHARE;
        let room = bounds.height() - self.bar;
        let shown = levels.listen(self.spectrum.as_ref(), self.bars, ctx);
        for (index, shown) in shown.iter().enumerate() {
            let tall = self.bar + room * shown;
            ctx.draw_quad(
                Quad::new(Rect::new(
                    bounds.x() + pitch * index as f32,
                    bounds.y() + (bounds.height() - tall) / 2.0,
                    self.bar,
                    tall,
                ))
                .fill(self.color)
                .radius(self.bar / 2.0),
            );
        }
    }

    fn same(&self, other: &Self) -> bool {
        same_spectrum(self.spectrum.as_ref(), other.spectrum.as_ref())
            && self.bars == other.bars
            && self.height == other.height
            && self.color == other.color
    }

    fn debug_name(&self) -> &'static str {
        "Waveform"
    }
}

#[must_use]
pub fn same_spectrum(mine: Option<&Spectrum>, theirs: Option<&Spectrum>) -> bool {
    match (mine, theirs) {
        (Some(mine), Some(theirs)) => mine.is(theirs),
        (None, None) => true,
        _ => false,
    }
}

fn follow(shown: f32, event: f32, elapsed: Duration) -> f32 {
    let pace = match event > shown {
        true => RISE,
        false => FALL,
    };
    let decay = (-elapsed.as_secs_f32() / pace.as_secs_f32()).exp();
    event + (shown - event) * decay
}

impl std::fmt::Debug for Waveform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waveform")
            .field("bars", &self.bars)
            .field("listening", &self.spectrum.is_some())
            .finish_non_exhaustive()
    }
}

impl View for Waveform {
    fn build(self) -> Element {
        Element::new(self)
    }
}
