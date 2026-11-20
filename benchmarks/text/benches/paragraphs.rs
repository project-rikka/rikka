use std::hint::black_box;
use std::path::PathBuf;

use divan::Bencher;
use divan::counter::CharsCount;
use text_paragraphs::{CosmicParagraphs, ParleyParagraphs, RikkaParagraphs};
use text_samples::paragraph_cases;

fn main() {
    divan::main();
}

fn names() -> Vec<&'static str> {
    paragraph_cases().iter().map(|case| case.name).collect()
}

fn text(name: &str) -> String {
    paragraph_cases()
        .into_iter()
        .find(|case| case.name == name)
        .expect("paragraph case should exist")
        .text
}

trait Typesetter {
    fn new(fonts: &[PathBuf]) -> Self;

    fn shape_and_lay(&mut self, text: &str) -> usize;

    fn relay(&mut self, wide: bool) -> usize;
}

struct Rikka {
    paragraphs: RikkaParagraphs,
    shaped: Option<rikka_text_layout::ShapedParagraph>,
}

impl Typesetter for Rikka {
    fn new(_fonts: &[PathBuf]) -> Self {
        Self {
            paragraphs: RikkaParagraphs::new(),
            shaped: None,
        }
    }

    fn shape_and_lay(&mut self, text: &str) -> usize {
        let shaped = self.paragraphs.shape(text);
        let rows = self.paragraphs.lay(&shaped, true);
        self.shaped = Some(shaped);
        rows
    }

    fn relay(&mut self, wide: bool) -> usize {
        let shaped = self.shaped.as_ref().expect("paragraph should be shaped");
        self.paragraphs.lay(shaped, wide)
    }
}

struct Cosmic(CosmicParagraphs);

impl Typesetter for Cosmic {
    fn new(fonts: &[PathBuf]) -> Self {
        Self(CosmicParagraphs::new(fonts))
    }

    fn shape_and_lay(&mut self, text: &str) -> usize {
        self.0.shape_and_lay(text)
    }

    fn relay(&mut self, wide: bool) -> usize {
        self.0.lay(wide)
    }
}

struct Parley(ParleyParagraphs);

impl Typesetter for Parley {
    fn new(fonts: &[PathBuf]) -> Self {
        Self(ParleyParagraphs::new(fonts))
    }

    fn shape_and_lay(&mut self, text: &str) -> usize {
        self.0.shape_and_lay(text)
    }

    fn relay(&mut self, wide: bool) -> usize {
        self.0.lay(wide)
    }
}

fn fonts(text: &str) -> Vec<PathBuf> {
    RikkaParagraphs::new().font_files(text)
}

#[divan::bench(types = [Rikka, Cosmic, Parley], args = names())]
fn shape_and_lay<T: Typesetter>(bencher: Bencher, name: &str) {
    let text = text(name);
    let mut typesetter = T::new(&fonts(&text));
    typesetter.shape_and_lay(&text);
    bencher
        .counter(CharsCount::of_str(&text))
        .bench_local(|| typesetter.shape_and_lay(black_box(&text)));
}

#[divan::bench(types = [Rikka, Cosmic, Parley], args = names())]
fn relayout<T: Typesetter>(bencher: Bencher, name: &str) {
    let text = text(name);
    let mut typesetter = T::new(&fonts(&text));
    typesetter.shape_and_lay(&text);
    let mut wide = false;
    bencher.counter(CharsCount::of_str(&text)).bench_local(|| {
        wide = !wide;
        typesetter.relay(wide)
    });
}
