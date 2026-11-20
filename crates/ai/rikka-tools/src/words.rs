use std::iter::Peekable;
use std::mem::take;
use std::str::Chars;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Reading {
    #[default]
    Lenient,
    Exact,
}

pub(crate) fn segments(command: &str) -> Vec<Vec<String>> {
    read(command, Reading::Lenient).unwrap_or_default()
}

pub(crate) fn exact_segments(command: &str) -> Option<Vec<Vec<String>>> {
    read(command, Reading::Exact)
}

fn read(command: &str, reading: Reading) -> Option<Vec<Vec<String>>> {
    let exact = reading == Reading::Exact;
    let mut segment = Segment {
        reading,
        ..Segment::default()
    };
    let mut letters = command.chars().peekable();

    while let Some(letter) = letters.next() {
        match letter {
            '\'' => segment.quoted(&mut letters)?,
            '"' => segment.doubly_quoted(&mut letters)?,
            '\\' => segment.escaped(&mut letters)?,
            '`' | '(' | ')' | '<' if exact => return None,
            '$' if exact && letters.peek() == Some(&'(') => return None,
            '{' | '}' if exact => segment.push(letter),
            '>' if exact => segment.nowhere(&mut letters)?,
            '<' | '>' => segment.thrown_away(&mut letters),
            '&' if exact => {
                letters.next_if_eq(&'&')?;
                segment.end_command();
            }
            '|' => {
                letters.next_if(|next| matches!(next, '|' | '&'));
                segment.end_command();
            }
            '&' | ';' | '\n' | '(' | ')' | '{' | '}' | '`' => {
                letters.next_if_eq(&letter);
                segment.end_command();
            }
            '#' if !segment.started => {
                while letters.next_if(|next| *next != '\n').is_some() {}
                segment.end_command();
            }
            letter if letter.is_whitespace() => segment.end_word(),
            letter => segment.push(letter),
        }
    }

    Some(segment.into_commands())
}

#[derive(Default)]
struct Segment {
    reading: Reading,
    commands: Vec<Vec<String>>,
    words: Vec<String>,
    word: String,
    started: bool,
}

impl Segment {
    fn is_exact(&self) -> bool {
        self.reading == Reading::Exact
    }

    fn push(&mut self, letter: char) {
        self.word.push(letter);
        self.started = true;
    }

    fn end_word(&mut self) {
        if self.started {
            self.words.push(take(&mut self.word));
            self.started = false;
        }
    }

    fn end_command(&mut self) {
        self.end_word();
        if !self.words.is_empty() {
            self.commands.push(take(&mut self.words));
        }
    }

    fn into_commands(mut self) -> Vec<Vec<String>> {
        self.end_command();
        self.commands
    }

    fn unclosed(&self) -> Option<()> {
        (!self.is_exact()).then_some(())
    }

    fn escaped(&mut self, letters: &mut Peekable<Chars<'_>>) -> Option<()> {
        match letters.next() {
            Some('\n') => {}
            Some(escaped) => self.push(escaped),
            None => return self.unclosed(),
        }
        Some(())
    }

    fn quoted(&mut self, letters: &mut Peekable<Chars<'_>>) -> Option<()> {
        self.started = true;
        for letter in letters.by_ref() {
            match letter {
                '\'' => return Some(()),
                letter => self.word.push(letter),
            }
        }
        self.unclosed()
    }

    fn doubly_quoted(&mut self, letters: &mut Peekable<Chars<'_>>) -> Option<()> {
        self.started = true;
        let exact = self.is_exact();
        while let Some(letter) = letters.next() {
            match letter {
                '"' => return Some(()),
                '`' if exact => return None,
                '$' if exact && letters.peek() == Some(&'(') => return None,
                '\\' => match letters.next() {
                    Some('\n') => {}
                    Some(escaped) => self.word.push(escaped),
                    None => return self.unclosed(),
                },
                letter => self.word.push(letter),
            }
        }
        self.unclosed()
    }

    fn thrown_away(&mut self, letters: &mut Peekable<Chars<'_>>) {
        match self.word.chars().all(|letter| letter.is_ascii_digit()) {
            true => {
                self.word.clear();
                self.started = false;
            }
            false => self.end_word(),
        }

        letters.next_if(|next| matches!(next, '>' | '&'));
        while letters.next_if(|next| matches!(next, ' ' | '\t')).is_some() {}
        while letters.next_if(|next| !next.is_whitespace()).is_some() {}
    }

    fn nowhere(&mut self, letters: &mut Peekable<Chars<'_>>) -> Option<()> {
        if self.started {
            if !self
                .word
                .chars()
                .all(|character| character.is_ascii_digit())
            {
                return None;
            }
            self.word.clear();
            self.started = false;
        }

        letters.next_if_eq(&'>');
        if letters.next_if_eq(&'&').is_some() {
            return letters.next_if(char::is_ascii_digit).map(drop);
        }

        while letters.next_if(|next| matches!(next, ' ' | '\t')).is_some() {}

        let mut target = String::new();
        while let Some(letter) = letters.next_if(|next| !next.is_whitespace()) {
            target.push(letter);
        }
        (target == "/dev/null").then_some(())
    }
}
