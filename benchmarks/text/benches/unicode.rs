use std::hint::black_box;

use divan::Bencher;
use divan::counter::BytesCount;
use text_unicode::{
    icu_graphemes, icu_lines, icu_nfc, icu_nfd, icu_words, rikka_bidi, rikka_graphemes,
    rikka_lines, rikka_nfc, rikka_nfd, rikka_words, unicode_case, unicode_cases, unicode_rs_bidi,
    unicode_rs_graphemes, unicode_rs_lines, unicode_rs_nfc, unicode_rs_nfd, unicode_rs_words,
};

fn main() {
    divan::main();
}

fn names() -> Vec<&'static str> {
    unicode_cases().iter().map(|case| case.name).collect()
}

fn text(name: &str) -> String {
    unicode_case(name).expect("unicode case should exist").text
}

trait Unicode {
    fn graphemes(text: &str) -> usize;

    fn words(text: &str) -> usize;

    fn lines(text: &str) -> usize;

    fn nfc(text: &str) -> usize;

    fn nfd(text: &str) -> usize;
}

struct Rikka;

impl Unicode for Rikka {
    fn graphemes(text: &str) -> usize {
        rikka_graphemes(text).len()
    }

    fn words(text: &str) -> usize {
        rikka_words(text).len()
    }

    fn lines(text: &str) -> usize {
        rikka_lines(text).len()
    }

    fn nfc(text: &str) -> usize {
        rikka_nfc(text).len()
    }

    fn nfd(text: &str) -> usize {
        rikka_nfd(text).len()
    }
}

struct Icu4x;

impl Unicode for Icu4x {
    fn graphemes(text: &str) -> usize {
        icu_graphemes(text).len()
    }

    fn words(text: &str) -> usize {
        icu_words(text).len()
    }

    fn lines(text: &str) -> usize {
        icu_lines(text).len()
    }

    fn nfc(text: &str) -> usize {
        icu_nfc(text).len()
    }

    fn nfd(text: &str) -> usize {
        icu_nfd(text).len()
    }
}

struct UnicodeRs;

impl Unicode for UnicodeRs {
    fn graphemes(text: &str) -> usize {
        unicode_rs_graphemes(text).len()
    }

    fn words(text: &str) -> usize {
        unicode_rs_words(text).len()
    }

    fn lines(text: &str) -> usize {
        unicode_rs_lines(text).len()
    }

    fn nfc(text: &str) -> usize {
        unicode_rs_nfc(text).len()
    }

    fn nfd(text: &str) -> usize {
        unicode_rs_nfd(text).len()
    }
}

fn run(bencher: Bencher, name: &str, operation: fn(&str) -> usize) {
    let text = text(name);
    bencher
        .counter(BytesCount::of_str(&text))
        .bench_local(|| operation(black_box(&text)));
}

#[divan::bench(types = [Rikka, Icu4x, UnicodeRs], args = names())]
fn graphemes<U: Unicode>(bencher: Bencher, name: &str) {
    run(bencher, name, U::graphemes);
}

#[divan::bench(types = [Rikka, Icu4x, UnicodeRs], args = names())]
fn words<U: Unicode>(bencher: Bencher, name: &str) {
    run(bencher, name, U::words);
}

#[divan::bench(types = [Rikka, Icu4x, UnicodeRs], args = names())]
fn line_breaks<U: Unicode>(bencher: Bencher, name: &str) {
    run(bencher, name, U::lines);
}

#[divan::bench(types = [Rikka, Icu4x, UnicodeRs], args = names())]
fn nfc<U: Unicode>(bencher: Bencher, name: &str) {
    run(bencher, name, U::nfc);
}

#[divan::bench(types = [Rikka, Icu4x, UnicodeRs], args = names())]
fn nfd<U: Unicode>(bencher: Bencher, name: &str) {
    run(bencher, name, U::nfd);
}

#[divan::bench(args = names())]
fn bidi_rikka(bencher: Bencher, name: &str) {
    let text = text(name);
    let mut bidi = rikka_text_unicode::Bidi::new();
    bencher
        .counter(BytesCount::of_str(&text))
        .bench_local(|| rikka_bidi(&mut bidi, black_box(&text)));
}

#[divan::bench(args = names())]
fn bidi_unicode_rs(bencher: Bencher, name: &str) {
    run(bencher, name, unicode_rs_bidi);
}
