use rikka_sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use rikka_i18n_catalog::{Args, Locale};

use crate::i18n::I18n;

static GLOBAL: LazyLock<RwLock<Arc<I18n>>> = LazyLock::new(|| RwLock::new(Arc::new(I18n::empty())));

static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn install(i18n: I18n) {
    *GLOBAL.write() = Arc::new(i18n);
    GENERATION.fetch_add(1, Ordering::Release);
}

pub fn global() -> Arc<I18n> {
    Arc::clone(&GLOBAL.read())
}

fn with_global<R>(read: impl FnOnce(&I18n) -> R) -> R {
    read(&GLOBAL.read())
}

pub fn set_locale(locale: Locale) {
    let switched = global().with_locale(locale);
    install(switched);
}

pub fn set_language(tag: &str) -> Option<Locale> {
    let requested: Locale = tag.parse().ok()?;
    let negotiated = with_global(|i18n| i18n.negotiate([&requested]))?;
    if negotiated != current_locale() {
        set_locale(negotiated.clone());
    }
    Some(negotiated)
}

pub fn current_locale() -> Locale {
    with_global(|i18n| i18n.locale().clone())
}

pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

pub fn translate(key: &str) -> Option<Arc<str>> {
    with_global(|i18n| i18n.translate(key))
}

pub fn tr(key: &str) -> Arc<str> {
    with_global(|i18n| i18n.tr(key))
}

pub(crate) fn tr_with(key: &str, args: &Args<'_>) -> Arc<str> {
    with_global(|i18n| i18n.tr_with(key, args))
}
