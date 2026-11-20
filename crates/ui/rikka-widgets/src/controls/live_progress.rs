use rikka_types::Color;
use rikka_ui::{Layout, PaintCtx, Style, Watchers, Widget};

use crate::control::ControlSize;
use crate::controls::progress::{Progress, ProgressState, progress};

#[must_use]
#[derive(Widget)]
pub struct LiveProgress {
    watchers: Watchers,
    read: Box<dyn Fn() -> f32>,
    size: ControlSize,
    color: Option<Color>,
    #[style]
    style: Style,
}

pub fn live_progress(watchers: &Watchers, read: impl Fn() -> f32 + 'static) -> LiveProgress {
    LiveProgress {
        watchers: watchers.clone(),
        read: Box::new(read),
        size: ControlSize::default(),
        color: None,
        style: Style::default(),
    }
}

impl LiveProgress {
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    fn bar(&self, value: f32) -> Progress {
        let bar = progress(value).control_size(self.size);
        match self.color {
            Some(color) => bar.color(color),
            None => bar,
        }
    }
}

impl Widget for LiveProgress {
    type State = ProgressState;

    fn layout(&self) -> Layout {
        self.bar(0.0).layout().with(&self.style)
    }

    fn paint(&self, state: &mut ProgressState, ctx: &mut PaintCtx<'_>) {
        self.watchers.watch(ctx.handle());
        self.bar((self.read)()).paint(state, ctx);
    }
}
