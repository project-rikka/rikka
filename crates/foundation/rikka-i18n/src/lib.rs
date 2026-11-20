mod builder;
mod global;
mod i18n;
mod label;
mod language;
mod loader;
mod macros;

pub use builder::I18nBuilder;
pub use global::{
    current_locale, generation, global, install, set_language, set_locale, tr, translate,
};
pub use i18n::I18n;
pub use label::Label;
pub use language::{Language, languages};
pub use loader::{LoadError, LoadReport, load_dir};
pub use rikka_i18n_catalog::{
    Args, Catalog, Cause, Error, IntoLocale, Locale, Message, PluralCategory, PluralKind,
    PluralOperands, Result, SyntaxError, Value, negotiate,
};
#[doc(hidden)]
pub use rikka_i18n_macros::catalogs;
