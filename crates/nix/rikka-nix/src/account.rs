#[must_use]
pub fn login() -> Option<String> {
    let name = rikka_dirs::passwd::current()?.name;
    named(&name).then_some(name)
}

fn named(login: &str) -> bool {
    !login.is_empty()
        && login
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}
