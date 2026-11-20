use std::borrow::Cow;

const ESCAPE: char = '\u{1b}';

const BELL: char = '\u{7}';

pub(crate) fn readable(text: &str) -> Cow<'_, str> {
    if !text
        .chars()
        .any(|character| character.is_control() && character != '\t')
    {
        return Cow::Borrowed(text);
    }
    let mut kept = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            ESCAPE => match characters.next() {
                Some('[') => skip_control_sequence(&mut characters),
                Some(']') => skip_operating_system_command(&mut characters),
                _ => {}
            },
            '\t' => kept.push('\t'),
            control if control.is_control() => {}
            printable => kept.push(printable),
        }
    }
    Cow::Owned(kept)
}

fn skip_control_sequence(characters: &mut impl Iterator<Item = char>) {
    for character in characters {
        if ('\u{40}'..='\u{7e}').contains(&character) {
            return;
        }
    }
}

fn skip_operating_system_command(characters: &mut std::iter::Peekable<impl Iterator<Item = char>>) {
    while let Some(character) = characters.next() {
        match character {
            BELL => return,
            ESCAPE if characters.peek() == Some(&'\\') => {
                characters.next();
                return;
            }
            _ => {}
        }
    }
}
