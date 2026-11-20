use rikka_text_font::{
    ChainedSequenceContext, ChainedSequenceRule, ClassDef, CoverageArray, Font, GlyphId,
    LayoutTable, OutlineFormat, SequenceContext, SubstSubtable, Tag,
};
use rikka_text_unicode::{GeneralCategory, Script};

use crate::engines::ShapedGlyph;

const PROBED_FEATURES: [&[u8; 4]; 5] = [b"rphf", b"pref", b"blwf", b"pstf", b"vatu"];
const DOTTED_CIRCLE: u32 = 0x25CC;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarfrustBug {
    ContextualProbeMisread,
    ControlCharacterTakenAsLetter,
    MissingGlyphExtents,
    RewindOverwritesGlyphs,
}

fn virama(script: Script) -> Option<u32> {
    Some(match script {
        Script::Devanagari => 0x094D,
        Script::Bengali => 0x09CD,
        Script::Gurmukhi => 0x0A4D,
        Script::Gujarati => 0x0ACD,
        Script::Oriya => 0x0B4D,
        Script::Tamil => 0x0BCD,
        Script::Telugu => 0x0C4D,
        Script::Kannada => 0x0CCD,
        Script::Malayalam => 0x0D4D,
        _ => return None,
    })
}

pub fn harfrust_bug(
    font: &Font<'_>,
    script: Script,
    text: &str,
    varied: bool,
    ours: &[ShapedGlyph],
    theirs: &[ShapedGlyph],
) -> Option<HarfrustBug> {
    if let Some(virama) = virama(script) {
        if misreads_contextual_probe(font, script, text, virama) {
            return Some(HarfrustBug::ContextualProbeMisread);
        }
        if left_matras_after_controls_only_differ(font, text, ours, theirs) {
            return Some(HarfrustBug::ControlCharacterTakenAsLetter);
        }
    }
    if lacks_mark_extents(font, text, varied) {
        return Some(HarfrustBug::MissingGlyphExtents);
    }
    repeats_glyph_after_rewind(ours, theirs).then_some(HarfrustBug::RewindOverwritesGlyphs)
}

fn repeats_glyph_after_rewind(ours: &[ShapedGlyph], theirs: &[ShapedGlyph]) -> bool {
    ours.iter()
        .zip(theirs)
        .position(|(a, b)| a.glyph != b.glyph)
        .is_some_and(|first| {
            first > 0
                && theirs[first].glyph == theirs[first - 1].glyph
                && ours[first - 1].glyph == theirs[first - 1].glyph
        })
}

fn lacks_mark_extents(font: &Font<'_>, text: &str, varied: bool) -> bool {
    font.gpos().is_none()
        && (varied || font.outline_format() != Some(OutlineFormat::TrueType))
        && text.chars().any(|character| {
            matches!(
                rikka_text_unicode::general_category(character),
                GeneralCategory::NonspacingMark
                    | GeneralCategory::SpacingMark
                    | GeneralCategory::EnclosingMark
            )
        })
}

fn follows_control(font: &Font<'_>, text: &str, previous: &ShapedGlyph) -> bool {
    let dotted_circle = font
        .charmap()
        .and_then(|charmap| charmap.map(DOTTED_CIRCLE))
        .map(|glyph| u32::from(glyph.0));
    dotted_circle == Some(previous.glyph)
        || text
            .get(previous.cluster as usize..)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|character| {
                rikka_text_unicode::general_category(character) == GeneralCategory::Control
            })
}

fn left_matras_after_controls_only_differ(
    font: &Font<'_>,
    text: &str,
    ours: &[ShapedGlyph],
    theirs: &[ShapedGlyph],
) -> bool {
    if ours.len() != theirs.len() {
        return false;
    }
    let mut differs = false;
    for index in 0..ours.len() {
        if ours[index] == theirs[index] {
            continue;
        }
        differs = true;
        let cluster = ours[index].cluster;
        let mut start = index;
        while start > 0 && ours[start - 1].cluster == cluster {
            start -= 1;
        }
        let explained = start > 0
            && theirs[index].cluster == cluster
            && theirs[index].unsafe_to_break
            && !ours[index].unsafe_to_break
            && follows_control(font, text, &ours[start - 1]);
        if !explained {
            return false;
        }
    }
    differs
}

fn probed_lookups(table: &LayoutTable<'_>) -> Vec<u16> {
    let features = table.features();
    let mut lookups: Vec<u16> = (0..features.len() as u16)
        .filter_map(|index| features.get(index))
        .filter(|feature| {
            PROBED_FEATURES
                .iter()
                .any(|tag| feature.tag() == Tag::new(tag))
        })
        .flat_map(|feature| feature.lookup_indices().iter().collect::<Vec<_>>())
        .collect();
    lookups.sort_unstable();
    lookups.dedup();
    lookups
}

fn covered(coverages: &CoverageArray<'_>, index: usize, glyph: u32) -> bool {
    coverages
        .get(index)
        .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
}

fn rule_inputs_match(input: &[u16], glyphs: &[u32], matches: impl Fn(u32, u16) -> bool) -> bool {
    glyphs.len() == input.len() + 1
        && input
            .iter()
            .zip(&glyphs[1..])
            .all(|(&value, &glyph)| matches(glyph, value))
}

fn class_of(class_def: &ClassDef<'_>, glyph: u32) -> u16 {
    class_def.get(GlyphId(glyph as u16))
}

fn chained_rule_matches_with_context(
    rule: &ChainedSequenceRule<'_>,
    glyphs: &[u32],
    matches: impl Fn(u32, u16) -> bool,
) -> bool {
    (!rule.backtrack().is_empty() || !rule.lookahead().is_empty())
        && rule_inputs_match(&rule.input().iter().collect::<Vec<_>>(), glyphs, matches)
}

fn subtable_diverges(subtable: &SubstSubtable<'_>, glyphs: &[u32], malayalam: bool) -> bool {
    let first = GlyphId(glyphs[0] as u16);
    match subtable {
        SubstSubtable::Context(SequenceContext::Coverages(coverages)) => {
            let input = coverages.input();
            if !covered(&input, 0, glyphs[0]) {
                return false;
            }
            let harfbuzz = glyphs.len() == input.len()
                && (1..glyphs.len()).all(|index| covered(&input, index, glyphs[index]));
            let harfrust = glyphs.len() == input.len() + 1
                && (0..input.len()).all(|index| covered(&input, index, glyphs[index + 1]));
            harfbuzz != harfrust
        }
        SubstSubtable::ChainContext(context) if !malayalam => match context {
            ChainedSequenceContext::Glyphs(rules) => rules
                .coverage()
                .get(first)
                .and_then(|index| rules.rule_set(index))
                .is_some_and(|set| {
                    set.iter().any(|rule| {
                        chained_rule_matches_with_context(&rule, glyphs, |glyph, value| {
                            glyph == u32::from(value)
                        })
                    })
                }),
            ChainedSequenceContext::Classes(classes) => {
                let input = classes.input_class_def();
                classes.coverage().contains(first)
                    && classes
                        .rule_set(class_of(&input, glyphs[0]))
                        .is_some_and(|set| {
                            set.iter().any(|rule| {
                                chained_rule_matches_with_context(&rule, glyphs, |glyph, class| {
                                    class_of(&input, glyph) == class
                                })
                            })
                        })
            }
            ChainedSequenceContext::Coverages(coverages) => {
                let input = coverages.input();
                (!coverages.backtrack().is_empty() || !coverages.lookahead().is_empty())
                    && glyphs.len() == input.len()
                    && (0..glyphs.len()).all(|index| covered(&input, index, glyphs[index]))
            }
        },
        _ => false,
    }
}

fn misreads_contextual_probe(font: &Font<'_>, script: Script, text: &str, virama: u32) -> bool {
    let Some(gsub) = font.gsub() else {
        return false;
    };
    let Some(charmap) = font.charmap() else {
        return false;
    };
    let glyph = |codepoint: u32| charmap.map(codepoint).map_or(0, |glyph| u32::from(glyph.0));
    let virama = glyph(virama);
    let glyphs: Vec<u32> = text
        .chars()
        .map(|character| glyph(character as u32))
        .collect();
    let mut sequences: Vec<Vec<u32>> = Vec::new();
    for &consonant in &glyphs {
        sequences.push(vec![virama, consonant]);
        sequences.push(vec![consonant, virama]);
    }
    for window in glyphs.windows(2).chain(glyphs.windows(3)) {
        sequences.push(window.to_vec());
    }
    let malayalam = script == Script::Malayalam;
    probed_lookups(&gsub).into_iter().any(|index| {
        gsub.lookup(index).is_some_and(|lookup| {
            lookup.subtables().any(|subtable| {
                sequences
                    .iter()
                    .any(|sequence| subtable_diverges(&subtable, sequence, malayalam))
            })
        })
    })
}
