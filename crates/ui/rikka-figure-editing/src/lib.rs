mod controls;
mod overlay;
mod typing;

pub use controls::{align_choice, font_choice};
pub use overlay::{paint_band, paint_dashed, paint_grips, paint_line, paint_loop, paint_typing};
pub use typing::{Typing, TypingOutcome, text_with_preedit};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
