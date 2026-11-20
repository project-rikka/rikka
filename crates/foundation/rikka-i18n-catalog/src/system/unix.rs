pub(crate) fn tag() -> Option<String> {
    Some(
        rikka_types::LocaleCategory::Messages
            .locale()?
            .name()
            .to_owned(),
    )
}
