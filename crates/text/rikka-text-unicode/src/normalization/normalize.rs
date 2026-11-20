use std::borrow::Cow;

use super::decomposition::{canonical_decomposition, compatibility_decomposition, compose};
use super::marks::Marks;
use crate::properties::{QuickCheck, properties};

const FIRST_DECOMPOSING: char = '\u{C0}';
const FIRST_COMPATIBLE: char = '\u{A0}';
const FIRST_COMBINING: char = '\u{300}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mapping {
    Canonical,
    Compatible,
}

impl Mapping {
    fn first_decomposing(self) -> char {
        match self {
            Self::Canonical => FIRST_DECOMPOSING,
            Self::Compatible => FIRST_COMPATIBLE,
        }
    }
}

pub fn is_nfd(text: &str) -> bool {
    let mut last_class = 0;
    for character in text.chars() {
        if character < FIRST_DECOMPOSING {
            last_class = 0;
            continue;
        }
        let properties = properties(character);
        let class = properties.combining_class();
        if properties.decomposes() || (class != 0 && last_class > class) {
            return false;
        }
        last_class = class;
    }
    true
}

pub fn is_nfc(text: &str) -> bool {
    match quick_check_nfc(text) {
        QuickCheck::Yes => true,
        QuickCheck::No => false,
        QuickCheck::Maybe => {
            let mut normalized = String::with_capacity(text.len());
            nfc_into(text, &mut normalized);
            normalized == text
        }
    }
}

pub fn nfd(text: &str) -> Cow<'_, str> {
    if is_nfd(text) {
        return Cow::Borrowed(text);
    }
    let mut normalized = String::with_capacity(text.len() + text.len() / 2);
    nfd_into(text, &mut normalized);
    Cow::Owned(normalized)
}

pub fn nfc(text: &str) -> Cow<'_, str> {
    match quick_check_nfc(text) {
        QuickCheck::Yes => Cow::Borrowed(text),
        QuickCheck::No | QuickCheck::Maybe => {
            let mut normalized = String::with_capacity(text.len());
            nfc_into(text, &mut normalized);
            unchanged_or(text, normalized)
        }
    }
}

pub fn nfkd(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        return Cow::Borrowed(text);
    }
    let mut normalized = String::with_capacity(text.len() + text.len() / 2);
    nfkd_into(text, &mut normalized);
    unchanged_or(text, normalized)
}

pub fn nfkc(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        return Cow::Borrowed(text);
    }
    let mut normalized = String::with_capacity(text.len());
    nfkc_into(text, &mut normalized);
    unchanged_or(text, normalized)
}

pub fn nfd_into(text: &str, output: &mut String) {
    decompose_into(text, Mapping::Canonical, output);
}

pub fn nfc_into(text: &str, output: &mut String) {
    compose_into(text, Mapping::Canonical, output);
}

pub fn nfkd_into(text: &str, output: &mut String) {
    decompose_into(text, Mapping::Compatible, output);
}

pub fn nfkc_into(text: &str, output: &mut String) {
    compose_into(text, Mapping::Compatible, output);
}

fn unchanged_or(text: &str, normalized: String) -> Cow<'_, str> {
    if normalized == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(normalized)
    }
}

fn decompose_into(text: &str, mapping: Mapping, output: &mut String) {
    let mut marks = Marks::new();
    decompose_ordered(text, mapping, &mut marks, |character, _| {
        output.push(character);
    });
}

fn compose_into(text: &str, mapping: Mapping, output: &mut String) {
    let mut marks = Marks::new();
    let mut composer = Composer::new(output);
    decompose_ordered(text, mapping, &mut marks, |character, class| {
        composer.feed(character, class);
    });
    composer.finish();
}

fn quick_check_nfc(text: &str) -> QuickCheck {
    let mut last_class = 0;
    let mut result = QuickCheck::Yes;
    for character in text.chars() {
        if character < FIRST_COMBINING {
            last_class = 0;
            continue;
        }
        let properties = properties(character);
        let class = properties.combining_class();
        if class != 0 && last_class > class {
            return QuickCheck::No;
        }
        match properties.nfc_quick_check() {
            QuickCheck::No => return QuickCheck::No,
            QuickCheck::Maybe => result = QuickCheck::Maybe,
            QuickCheck::Yes => {}
        }
        last_class = class;
    }
    result
}

fn decompose_ordered(
    text: &str,
    mapping: Mapping,
    marks: &mut Marks,
    mut emit: impl FnMut(char, u8),
) {
    let first_decomposing = mapping.first_decomposing();
    let mut accept = |character: char, class: u8, marks: &mut Marks| {
        if class == 0 {
            flush(marks, &mut emit);
            emit(character, 0);
        } else {
            marks.push(class, character);
        }
    };
    for character in text.chars() {
        if character < first_decomposing {
            accept(character, 0, marks);
            continue;
        }
        let compatible = match mapping {
            Mapping::Canonical => None,
            Mapping::Compatible => compatibility_decomposition(character),
        };
        if let Some(parts) = compatible {
            for part in parts.iter().filter_map(|&part| char::from_u32(part)) {
                accept(part, crate::properties::combining_class(part), marks);
            }
            continue;
        }
        let properties = properties(character);
        if properties.decomposes() {
            for part in canonical_decomposition(character) {
                accept(part, crate::properties::combining_class(part), marks);
            }
        } else {
            accept(character, properties.combining_class(), marks);
        }
    }
    flush(marks, &mut emit);
}

fn flush(marks: &mut Marks, emit: &mut impl FnMut(char, u8)) {
    if marks.as_slice().is_empty() {
        return;
    }
    marks.sort_canonically();
    for &(class, character) in marks.as_slice() {
        emit(character, class);
    }
    marks.clear();
}

struct Composer<'a> {
    output: &'a mut String,
    starter: Option<char>,
    pending: Marks,
}

impl<'a> Composer<'a> {
    fn new(output: &'a mut String) -> Self {
        Self {
            output,
            starter: None,
            pending: Marks::new(),
        }
    }

    #[inline]
    fn feed(&mut self, character: char, class: u8) {
        if let Some(starter) = self.starter {
            let blocked = self
                .pending
                .last_class()
                .is_some_and(|last| last == 0 || last >= class);
            if !blocked && let Some(composite) = compose(starter, character) {
                self.starter = Some(composite);
                return;
            }
        }
        if class == 0 {
            self.flush();
            self.starter = Some(character);
        } else {
            self.pending.push(class, character);
        }
    }

    fn flush(&mut self) {
        if let Some(starter) = self.starter.take() {
            self.output.push(starter);
        }
        for &(_, character) in self.pending.as_slice() {
            self.output.push(character);
        }
        self.pending.clear();
    }

    fn finish(mut self) {
        self.flush();
    }
}
