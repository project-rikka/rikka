use rikka_types::Rect;
use rikka_ui::{PaintCtx, Quad, palette};

const SHORTEST_THUMB: f32 = 24.0;
const SCROLLS: f64 = 1e-3;
const THUMB: f32 = 5.0;
const LIT_THUMB: f32 = 8.0;
const RESTING: f32 = 0.4;
const LIT: f32 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollExtent {
    pub at: f64,
    pub shown: f64,
    pub content: f64,
}

impl ScrollExtent {
    #[must_use]
    pub fn reach(self) -> f64 {
        (self.content - self.shown).max(0.0)
    }

    #[must_use]
    pub fn scrolls(self) -> bool {
        self.reach() > self.shown * SCROLLS
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub start: f32,
    pub length: f32,
}

impl Thumb {
    #[must_use]
    pub fn holds(self, at: f32) -> bool {
        at >= self.start && at <= self.start + self.length
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollTrack {
    pub extent: ScrollExtent,
    pub from: f32,
    pub length: f32,
}

impl ScrollTrack {
    #[must_use]
    pub fn thumb(self) -> Thumb {
        let ScrollExtent { at, shown, content } = self.extent;
        let share = match content > 0.0 {
            true => (shown / content).min(1.0) as f32,
            false => 1.0,
        };
        let length = (self.length * share).max(SHORTEST_THUMB).min(self.length);
        let reach = self.extent.reach();
        let start = match reach > 0.0 {
            true => (self.length - length) * (at / reach).clamp(0.0, 1.0) as f32,
            false => 0.0,
        };
        Thumb {
            start: self.from + start,
            length,
        }
    }

    #[must_use]
    pub fn grip(self, at: f32) -> f32 {
        let thumb = self.thumb();
        match thumb.holds(at) {
            true => at - thumb.start,
            false => thumb.length / 2.0,
        }
    }

    #[must_use]
    pub fn at_for(self, start: f32) -> f64 {
        let travel = self.length - self.thumb().length;
        if travel <= 0.0 {
            return 0.0;
        }
        f64::from(((start - self.from) / travel).clamp(0.0, 1.0)) * self.extent.reach()
    }
}

pub fn paint_thumb(
    ctx: &mut PaintCtx<'_>,
    track: Rect,
    orientation: Orientation,
    thumb: Thumb,
    lit: bool,
) {
    let (across, alpha) = match lit {
        true => (LIT_THUMB, LIT),
        false => (THUMB, RESTING),
    };
    let rect = match orientation {
        Orientation::Horizontal => Rect::new(
            thumb.start,
            track.center().y - across / 2.0,
            thumb.length,
            across,
        ),
        Orientation::Vertical => Rect::new(
            track.center().x - across / 2.0,
            thumb.start,
            across,
            thumb.length,
        ),
    };
    ctx.draw_quad(
        Quad::new(rect)
            .fill(palette().text_muted.scale_alpha(alpha))
            .radius(across / 2.0),
    );
}
