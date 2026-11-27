mod browser;
mod shelving;
mod thumbnail;
mod view;

pub use browser::{Browser, Popup};
pub use shelving::{Desk, Painter, Painting, Performer, Picture, Reach, Shelving, Thumbnailing};
pub use thumbnail::variant;
pub use view::{WIDTH, library};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
