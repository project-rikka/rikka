use rikka_dirs::passwd;

pub(crate) fn name() -> String {
    passwd::current()
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_default())
}
