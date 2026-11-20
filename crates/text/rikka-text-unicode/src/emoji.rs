use crate::properties::properties;

pub const TEXT_PRESENTATION_SELECTOR: char = '\u{FE0E}';
pub const EMOJI_PRESENTATION_SELECTOR: char = '\u{FE0F}';
const ZERO_WIDTH_JOINER: char = '\u{200D}';
const COMBINING_KEYCAP: char = '\u{20E3}';
const CANCEL_TAG: char = '\u{E007F}';
const TAG_SPECS: std::ops::RangeInclusive<char> = '\u{E0020}'..='\u{E007E}';

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmojiPresentation {
    Text,
    Emoji,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmojiSequenceKind {
    Character,
    Presentation,
    Keycap,
    Modifier,
    Flag,
    Tag,
    Zwj,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmojiSequence {
    pub len: usize,
    pub kind: EmojiSequenceKind,
}

pub fn emoji_sequence(text: &str) -> Option<EmojiSequence> {
    let mut sequence = element(text)?;
    while let Some(rest) = text[sequence.len..].strip_prefix(ZERO_WIDTH_JOINER) {
        let Some(next) = element(rest) else {
            break;
        };
        sequence = EmojiSequence {
            len: sequence.len + ZERO_WIDTH_JOINER.len_utf8() + next.len,
            kind: EmojiSequenceKind::Zwj,
        };
    }
    Some(sequence)
}

pub fn emoji_presentation(cluster: &str) -> Option<EmojiPresentation> {
    let sequence = emoji_sequence(cluster)?;
    if sequence.kind != EmojiSequenceKind::Character {
        return Some(EmojiPresentation::Emoji);
    }
    let mut characters = cluster.chars();
    let base = characters.next()?;
    Some(
        if characters.next() != Some(TEXT_PRESENTATION_SELECTOR)
            && properties(base).is_emoji_presentation()
        {
            EmojiPresentation::Emoji
        } else {
            EmojiPresentation::Text
        },
    )
}

pub fn basic_emoji(text: &str) -> Option<EmojiSequence> {
    let mut characters = text.chars();
    let base = characters.next()?;
    let base_properties = properties(base);
    if !base_properties.is_emoji() || base_properties.is_regional_indicator() {
        return None;
    }
    match characters.next() {
        Some(EMOJI_PRESENTATION_SELECTOR)
            if base_properties.is_emoji_presentation() || !base_properties.is_emoji_component() =>
        {
            Some(EmojiSequence {
                len: base.len_utf8() + EMOJI_PRESENTATION_SELECTOR.len_utf8(),
                kind: EmojiSequenceKind::Presentation,
            })
        }
        Some(TEXT_PRESENTATION_SELECTOR) => None,
        _ if base_properties.is_emoji_presentation() => Some(EmojiSequence {
            len: base.len_utf8(),
            kind: EmojiSequenceKind::Character,
        }),
        _ => None,
    }
}

fn element(text: &str) -> Option<EmojiSequence> {
    let base = text.chars().next()?;
    let base_properties = properties(base);
    if !base_properties.is_emoji() {
        return None;
    }
    let mut len = base.len_utf8();
    let rest = &text[len..];
    if base_properties.is_regional_indicator() {
        if let Some(second) = rest
            .chars()
            .next()
            .filter(|&c| properties(c).is_regional_indicator())
        {
            return Some(EmojiSequence {
                len: len + second.len_utf8(),
                kind: EmojiSequenceKind::Flag,
            });
        }
        return Some(EmojiSequence {
            len,
            kind: EmojiSequenceKind::Character,
        });
    }
    if matches!(base, '0'..='9' | '#' | '*') {
        let keycap = rest
            .strip_prefix(EMOJI_PRESENTATION_SELECTOR)
            .unwrap_or(rest);
        if keycap.starts_with(COMBINING_KEYCAP) {
            return Some(EmojiSequence {
                len: text.len() - keycap.len() + COMBINING_KEYCAP.len_utf8(),
                kind: EmojiSequenceKind::Keycap,
            });
        }
    }
    let mut kind = EmojiSequenceKind::Character;
    match rest.chars().next() {
        Some(modifier)
            if base_properties.is_emoji_modifier_base()
                && properties(modifier).is_emoji_modifier() =>
        {
            len += modifier.len_utf8();
            kind = EmojiSequenceKind::Modifier;
        }
        Some(EMOJI_PRESENTATION_SELECTOR) => {
            len += EMOJI_PRESENTATION_SELECTOR.len_utf8();
            kind = EmojiSequenceKind::Presentation;
        }
        _ => {}
    }
    if let Some(tagged) = tag_sequence(&text[len..]) {
        len += tagged;
        kind = EmojiSequenceKind::Tag;
    }
    Some(EmojiSequence { len, kind })
}

fn tag_sequence(text: &str) -> Option<usize> {
    let specification = text
        .char_indices()
        .find(|&(_, character)| !TAG_SPECS.contains(&character))
        .map_or(text.len(), |(offset, _)| offset);
    if specification == 0 {
        return None;
    }
    text[specification..]
        .starts_with(CANCEL_TAG)
        .then_some(specification + CANCEL_TAG.len_utf8())
}
