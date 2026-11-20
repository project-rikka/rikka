mod card;
mod editing;
mod editor_canvas;
mod feature;
mod history;
mod overlay;
mod panel;
mod session;
mod shared;
mod sizes;
mod stage;
mod view;
mod wallpaper_loader;

pub use feature::DeskEditor;
use rikka_ui::DurationToken;

const EXIT_DURATION: DurationToken = DurationToken::Medium1;
