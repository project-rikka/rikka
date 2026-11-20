mod error;
mod export;
mod format;
mod name;
mod numbers;
mod parameter;
mod rgb;
mod shades;
mod spaces;
mod template;

pub use error::{Error, Result};
pub use export::{Document, Grouping, export};
pub use format::{NEW_FORMAT, template_named};
pub use name::ColorName;
pub use parameter::{Formatter, Parameter};
pub use rgb::Rgb;
pub use shades::Shades;
pub use spaces::{Cmyk, Hsi, Hsl, Hsv, Hwb, Lab, Natural, Oklab, Oklch, Xyz};
pub use template::Template;

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
