use rikka_session::wallpaper::Reserved;
use rikka_theme::Theme;
use shell_kit::taskbar::Shape;

#[must_use]
pub(crate) fn reserved(taskbar: Option<Shape>, theme: &Theme) -> Reserved {
    match (taskbar, theme.translucency().is_translucent()) {
        (Some(shape), false) => {
            let strip = shape.strip();
            Reserved {
                top: strip.top(),
                right: strip.right(),
                bottom: strip.bottom(),
                left: strip.left(),
            }
        }
        (None, _) | (Some(_), true) => Reserved::default(),
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
