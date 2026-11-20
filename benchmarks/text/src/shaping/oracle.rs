use rikka_test_fonts::find_loaded_font;

use crate::corpus::{Case, cases};
use crate::engines::{HarfrustEngine, Prepared, RikkaEngine, ShapedGlyph};
use crate::harfrust_divergence::{HarfrustBug, harfrust_bug};
use crate::random_texts::{Profile, TextGenerator};

#[derive(Debug, Default)]
pub struct GroupReport {
    pub passed: usize,
    pub skipped: usize,
    pub harfrust_bugs: usize,
    pub failures: Vec<String>,
}

impl GroupReport {
    pub fn total(&self) -> usize {
        self.passed + self.failures.len()
    }

    fn record(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Unavailable => self.skipped += 1,
            Outcome::HarfrustBug(_) => self.harfrust_bugs += 1,
            Outcome::Identical => self.passed += 1,
            Outcome::Different(failure) => self.failures.push(failure),
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Unavailable,
    HarfrustBug(HarfrustBug),
    Identical,
    Different(String),
}

fn describe(glyphs: &[ShapedGlyph]) -> String {
    glyphs
        .iter()
        .map(|glyph| {
            format!(
                "[{}={} +{} @{},{}{}{}]",
                glyph.glyph,
                glyph.cluster,
                glyph.x_advance,
                glyph.x_offset,
                glyph.y_offset,
                if glyph.unsafe_to_break { " B" } else { "" },
                if glyph.unsafe_to_concat { " C" } else { "" },
            )
        })
        .collect()
}

fn first_difference(ours: &[ShapedGlyph], theirs: &[ShapedGlyph]) -> usize {
    ours.iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()))
}

fn window(glyphs: &[ShapedGlyph], at: usize) -> &[ShapedGlyph] {
    let start = at.saturating_sub(2).min(glyphs.len());
    let end = (at + 4).min(glyphs.len());
    &glyphs[start..end]
}

pub fn compare(case: &Case, produce_concat: bool) -> Outcome {
    compare_text(case, case.text, produce_concat)
}

pub fn compare_text(case: &Case, text: &str, produce_concat: bool) -> Outcome {
    let Some(prepared) = find_loaded_font(case.font)
        .and_then(|data| Prepared::new(case, text, data, produce_concat))
    else {
        return Outcome::Unavailable;
    };
    let (Some(ours), Some(theirs)) = (RikkaEngine::new(&prepared), HarfrustEngine::new(&prepared))
    else {
        return Outcome::Unavailable;
    };
    let (ours, theirs) = (ours.shape_corpus_text(), theirs.shape_corpus_text());
    if ours == theirs {
        return Outcome::Identical;
    }
    if let Some(bug) = prepared.font().and_then(|font| {
        harfrust_bug(
            &font,
            prepared.properties.script,
            text,
            !prepared.location().coords().is_empty(),
            &ours,
            &theirs,
        )
    }) {
        return Outcome::HarfrustBug(bug);
    }
    Outcome::Different(format!(
        "{} [{}#{}] {:?} script={:?} rtl={:?} lang={:?} features={:?} variations={:?} concat={} first difference at {} of {}/{}\n  rikka-text-shape: {}\n  harfrust:         {}",
        case.group,
        case.font,
        case.face,
        text,
        prepared.properties.script,
        prepared.properties.direction,
        case.language,
        case.features,
        case.variations,
        produce_concat,
        first_difference(&ours, &theirs),
        ours.len(),
        theirs.len(),
        describe(window(&ours, first_difference(&ours, &theirs))),
        describe(window(&theirs, first_difference(&ours, &theirs))),
    ))
}

pub fn check_group(group: &str) -> GroupReport {
    let mut report = GroupReport::default();
    for case in cases().iter().filter(|case| case.group == group) {
        for produce_concat in [false, true] {
            report.record(compare(case, produce_concat));
        }
    }
    report
}

pub fn check_random(profile: &Profile, texts_per_font: usize, seed: u64) -> GroupReport {
    let mut report = GroupReport::default();
    let mut generator = TextGenerator::new(seed);
    const INSTANCES: [&[(&[u8; 4], f32)]; 4] = [
        &[(b"wght", 250.0)],
        &[(b"wght", 780.0), (b"wdth", 87.5)],
        &[(b"wght", 530.0), (b"opsz", 20.0)],
        &[(b"wdth", 70.0)],
    ];
    const FEATURES: [&str; 8] = [
        "",
        "liga=0",
        "kern=0",
        "smcp,c2sc",
        "dlig",
        "liga[2:5]=0,kern[1:3]=0",
        "ss01,ss02,salt",
        "-calt,+frac",
    ];
    for font in profile.fonts {
        for index in 0..texts_per_font {
            let mut case =
                Case::new(profile.name, font, "").features(FEATURES[index / 4 % FEATURES.len()]);
            if index % 4 >= 2 {
                case = case.bot_eot();
            }
            if index % 3 == 1 {
                case = case.variations(INSTANCES[index / 3 % INSTANCES.len()]);
            }
            let case = &case;
            let text = generator.text(profile, 24);
            if moves_nikhahit_over_several_marks(&text) {
                report.skipped += 1;
                continue;
            }
            report.record(compare_text(case, &text, index % 2 == 1));
        }
    }
    report
}

fn is_above_base_mark(codepoint: u32) -> bool {
    matches!(codepoint & !0x80, 0x0E31 | 0x0E34..=0x0E37 | 0x0E3B | 0x0E47..=0x0E4E)
}

fn moves_nikhahit_over_several_marks(text: &str) -> bool {
    let codepoints: Vec<u32> = text.chars().map(u32::from).collect();
    codepoints.iter().enumerate().any(|(index, &codepoint)| {
        codepoint & !0x80 == 0x0E33
            && index >= 2
            && is_above_base_mark(codepoints[index - 1])
            && is_above_base_mark(codepoints[index - 2])
    })
}
