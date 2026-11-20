mod cloth;
mod easy;
mod emitter;
mod fluid;
mod gesture;
mod handles;
mod overlay;
mod screen;
mod strokes;
mod wind;
mod wording;

pub use easy::Marker;
pub use gesture::Gesture;
pub use screen::{Aim, ClothAim, FigureAim, Marked, Screen};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
