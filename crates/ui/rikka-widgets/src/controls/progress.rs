use std::time::{Duration, Instant};

use rikka_animation::Easing;
use rikka_types::{Color, Rect};
use rikka_ui::{Layout, Length, PaintCtx, Quad, Style, Widget, motion};

use crate::control::ControlSize;
use crate::controls::spinner::looped;

const SWEEP: Duration = Duration::from_millis(1500);

#[must_use]
#[derive(Widget)]
pub struct Progress {
    #[debug]
    value: Option<f32>,
    size: ControlSize,
    color: Option<Color>,
    #[style]
    style: Style,
}

pub fn progress(value: f32) -> Progress {
    Progress {
        value: Some(value.clamp(0.0, 1.0)),
        size: ControlSize::default(),
        color: None,
        style: Style::default(),
    }
}

pub fn progress_indeterminate() -> Progress {
    Progress {
        value: None,
        ..progress(0.0)
    }
}

impl Progress {
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn thickness(&self) -> f32 {
        match self.size {
            ControlSize::Small => 4.0,
            ControlSize::Medium => 6.0,
            ControlSize::Large => 10.0,
        }
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ProgressState {
    since: Option<Instant>,
}

impl Widget for Progress {
    type State = ProgressState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(self.thickness()))
            .fixed()
            .with(&self.style)
    }

    fn paint(&self, state: &mut ProgressState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let thickness = bounds.height();
        let radius = thickness / 2.0;
        let fill = self.color.unwrap_or(ctx.theme().palette().accent);

        ctx.draw_quad(
            Quad::new(bounds)
                .fill(ctx.theme().palette().surface_variant)
                .radius(radius),
        );

        let (start, end) = match self.value {
            Some(value) => (0.0, ctx.animate_curve("filled", value, motion::FILL)),
            None => {
                let phase = looped(ctx, &mut state.since, SWEEP);
                (
                    Easing::EmphasizedAccel.transform(phase),
                    Easing::EmphasizedDecel.transform(phase),
                )
            }
        };
        let (x, width) = (
            bounds.x() + start * bounds.width(),
            (end - start) * bounds.width(),
        );
        if width <= 0.0 {
            return;
        }
        ctx.draw_quad(
            Quad::new(Rect::new(x, bounds.y(), width, thickness))
                .fill(fill)
                .radius(radius),
        );
    }

    fn same(&self, other: &Self) -> bool {
        self.value == other.value
            && self.size == other.size
            && self.color == other.color
            && self.style == other.style
    }
}
