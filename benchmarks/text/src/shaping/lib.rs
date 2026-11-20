mod corpus;
mod engines;
mod harfbuzz;
mod harfrust_divergence;
mod oracle;
mod random_texts;
mod syllabic_corpus;
mod syllabic_profiles;

pub use corpus::{BENCHMARK_TEXTS, Case, cases};
pub use engines::{HarfrustEngine, Prepared, RikkaEngine, ShapedGlyph};
pub use harfbuzz::HarfBuzzEngine;
pub use harfrust_divergence::{HarfrustBug, harfrust_bug};
pub use oracle::{GroupReport, Outcome, check_group, check_random, compare, compare_text};
pub use random_texts::{PROFILES, Profile, TextGenerator, profile};
pub use syllabic_corpus::{SYLLABIC_CORPORA, ScriptCorpus, USE_SAMPLES};
pub use syllabic_profiles::SYLLABIC_PROFILES;
