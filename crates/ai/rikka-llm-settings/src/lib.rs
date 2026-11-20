mod accounts;
mod card;
mod endpoint_form;
mod language;
mod llm_settings;
mod model_editor;
mod models;
mod notice;
mod prelude;
mod rows;
mod signing;
mod usage;
mod wording;

pub use llm_settings::{LlmSettings, llm_settings};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
