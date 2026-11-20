use discord_core::{Emoji, Inline};
use rikka_text_unicode::{EmojiSequenceKind, basic_emoji, emoji_sequence, is_regional_indicator};

pub(crate) fn split_emoji(text: &str, out: &mut Vec<Inline>) {
    let mut plain_from = 0;
    let mut at = 0;
    while at < text.len() {
        if let Some(length) = emoji_at(&text[at..]) {
            if plain_from < at {
                out.push(Inline::Text(text[plain_from..at].to_owned()));
            }
            out.push(Inline::Emoji(Emoji::unicode(&text[at..at + length])));
            at += length;
            plain_from = at;
            continue;
        }
        at += text[at..].chars().next().map_or(1, char::len_utf8);
    }
    if plain_from < text.len() {
        out.push(Inline::Text(text[plain_from..].to_owned()));
    }
}

fn emoji_at(text: &str) -> Option<usize> {
    let sequence = emoji_sequence(text)?;
    match sequence.kind {
        EmojiSequenceKind::Character | EmojiSequenceKind::Presentation => basic_emoji(text)
            .map(|basic| basic.len)
            .or_else(|| regional_indicator(text)),
        EmojiSequenceKind::Keycap
        | EmojiSequenceKind::Modifier
        | EmojiSequenceKind::Flag
        | EmojiSequenceKind::Tag
        | EmojiSequenceKind::Zwj => Some(sequence.len),
    }
}

fn regional_indicator(text: &str) -> Option<usize> {
    text.chars()
        .next()
        .filter(|&letter| is_regional_indicator(letter))
        .map(char::len_utf8)
}
