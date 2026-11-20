#[must_use]
pub(crate) fn is_name(letter: char) -> bool {
    letter.is_alphanumeric() || matches!(letter, '-' | '_')
}

#[must_use]
pub(crate) fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_name)
}
