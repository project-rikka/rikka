#[must_use]
pub fn number(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}

#[must_use]
pub fn numbers(text: &str) -> Vec<u32> {
    text.split(|character: char| !character.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}
