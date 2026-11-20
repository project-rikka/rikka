#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let allowed = name
        .chars()
        .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-');
    allowed
        && !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

#[must_use]
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[must_use]
pub fn truncate(text: &str, most: usize) -> Option<String> {
    if text.chars().count() <= most {
        return None;
    }

    let at = text
        .char_indices()
        .nth(most - 1)
        .map_or(text.len(), |(at, _)| at);
    Some(format!("{}…", &text[..at]))
}
