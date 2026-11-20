use rikka_theme::Theme;
use shell_kit::taskbar::Shape;

#[must_use]
pub(crate) fn reserved(taskbar: Option<Shape>, theme: &Theme) -> f32 {
    match theme.translucency().is_translucent() {
        true => 0.0,
        false => taskbar.map_or(0.0, Shape::reserved),
    }
}

mod canvas;
mod crossfade;
mod feature;
mod service;
mod shown_on;
mod transition;

pub use feature::Backdrop;
pub use shown_on::{Decision, ShownOn};
pub use transition::{CROSSING, CROSSING_CURVE};
