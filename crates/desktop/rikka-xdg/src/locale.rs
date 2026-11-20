#[must_use]
pub fn locales() -> Vec<String> {
    match rikka_types::LocaleCategory::Messages.locale() {
        Some(locale) => widen(locale.name()),
        None => Vec::new(),
    }
}

#[must_use]
pub fn language() -> Option<String> {
    locales().into_iter().find(|tag| !tag.contains('@'))
}

fn widen(locale: &str) -> Vec<String> {
    let (locale, modifier) = match locale.split_once('@') {
        Some((locale, modifier)) => (locale, Some(modifier)),
        None => (locale, None),
    };
    let locale = locale.split('.').next().unwrap_or(locale);
    let language = locale.split('_').next().unwrap_or(locale);

    let mut candidates = Vec::new();
    if let Some(modifier) = modifier {
        candidates.push(format!("{locale}@{modifier}"));
    }
    candidates.push(locale.to_owned());
    if language != locale {
        if let Some(modifier) = modifier {
            candidates.push(format!("{language}@{modifier}"));
        }
        candidates.push(language.to_owned());
    }
    candidates
}
