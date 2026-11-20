use std::borrow::Cow;

const FULL_WIDTH_OFFSET: u32 = 0xFEE0;

pub(crate) fn fold_width(character: char) -> char {
    match character {
        '\u{FF01}'..='\u{FF5E}' => {
            char::from_u32(character as u32 - FULL_WIDTH_OFFSET).unwrap_or(character)
        }
        '\u{3000}' => ' ',
        other => other,
    }
}

pub(crate) fn fold(character: char) -> char {
    let character = fold_width(character);
    if character.is_ascii() {
        return character.to_ascii_lowercase();
    }
    let mut lower = character.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        (Some(first), Some(_)) if first.is_ascii() => first,
        _ => character,
    }
}

pub(crate) fn fold_into(target: &mut String, text: &str) {
    target.extend(text.chars().map(fold));
}

pub(crate) fn folded(text: &str) -> String {
    let mut target = String::with_capacity(text.len());
    fold_into(&mut target, text);
    target
}

pub(crate) fn width_folded(text: &str) -> Cow<'_, str> {
    if text
        .chars()
        .all(|character| fold_width(character) == character)
    {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.chars().map(fold_width).collect())
}
