use std::hint::black_box;

use divan::Bencher;
use divan::counter::CharsCount;
use rikka_test_fonts::find_loaded_font;
use text_shaping::{BENCHMARK_TEXTS, HarfBuzzEngine, HarfrustEngine, Prepared, RikkaEngine};

fn main() {
    divan::main();
}

fn names() -> Vec<&'static str> {
    BENCHMARK_TEXTS
        .iter()
        .filter(|case| find_loaded_font(case.font).is_some())
        .map(|case| case.group)
        .collect()
}

fn prepared(name: &str) -> &'static Prepared {
    let case = BENCHMARK_TEXTS
        .iter()
        .find(|case| case.group == name)
        .expect("case should exist");
    let data = find_loaded_font(case.font).expect("font should be installed");
    let prepared = Prepared::new(case, case.text, data, false).expect("font should parse");
    Box::leak(Box::new(prepared))
}

trait Shaper: Sized {
    fn new(prepared: &'static Prepared) -> Self;

    fn shape(&mut self, text: &str) -> usize;
}

struct Rikka {
    engine: RikkaEngine<'static>,
    buffer: rikka_text_shape::Buffer,
}

impl Shaper for Rikka {
    fn new(prepared: &'static Prepared) -> Self {
        let engine = RikkaEngine::new(prepared).expect("rikka-text-shape should accept the font");
        let buffer = engine.buffer();
        Self { engine, buffer }
    }

    fn shape(&mut self, text: &str) -> usize {
        self.engine.shape(&mut self.buffer, text)
    }
}

struct HarfBuzz(HarfBuzzEngine);

impl Shaper for HarfBuzz {
    fn new(prepared: &'static Prepared) -> Self {
        Self(HarfBuzzEngine::new(prepared).expect("HarfBuzz should accept the font"))
    }

    fn shape(&mut self, text: &str) -> usize {
        self.0.shape(text)
    }
}

struct Harfrust {
    engine: &'static HarfrustEngine<'static>,
    shaper: harfrust::Shaper<'static>,
    plan: harfrust::ShapePlan,
    spare: Option<harfrust::UnicodeBuffer>,
}

impl Shaper for Harfrust {
    fn new(prepared: &'static Prepared) -> Self {
        let engine: &'static HarfrustEngine<'static> = Box::leak(Box::new(
            HarfrustEngine::new(prepared).expect("harfrust should accept the font"),
        ));
        let shaper = engine.shaper();
        let plan = engine.plan(&shaper);
        Self {
            engine,
            shaper,
            plan,
            spare: Some(harfrust::UnicodeBuffer::new()),
        }
    }

    fn shape(&mut self, text: &str) -> usize {
        let buffer = self.spare.take().unwrap_or_default();
        let output = self.engine.shape(&self.shaper, &self.plan, text, buffer);
        let glyphs = output.len();
        self.spare = Some(output.clear());
        glyphs
    }
}

#[divan::bench(types = [Rikka, HarfBuzz, Harfrust], args = names())]
fn shape<S: Shaper>(bencher: Bencher, name: &str) {
    let prepared = prepared(name);
    let text = prepared.text.as_str();
    let mut shaper = S::new(prepared);
    bencher
        .counter(CharsCount::of_str(text))
        .bench_local(|| shaper.shape(black_box(text)));
}
