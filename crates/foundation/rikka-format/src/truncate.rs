use std::borrow::Cow;

#[must_use]
pub fn truncate(text: &str, max_chars: usize) -> Cow<'_, str> {
    if text.chars().count() <= max_chars {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.chars().take(max_chars).chain(['…']).collect())
}
