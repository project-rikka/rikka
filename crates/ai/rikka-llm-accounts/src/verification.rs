use rikka_i18n::{Label, tr};

#[must_use]
pub fn verification_label() -> Label {
    tr!("llm.verify")
}

pub fn open_verification(url: &str) {
    if let Err(error) = rikka_open::open(url) {
        tracing::warn!("failed to open account verification page: {error}");
    }
}
