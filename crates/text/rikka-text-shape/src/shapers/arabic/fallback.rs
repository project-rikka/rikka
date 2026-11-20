use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::digest::Digest;
use crate::face::font_context::FontContext;
use crate::face::lookup_filter::LookupFilter;
use crate::layout::apply_context::ApplyContext;
use crate::layout::dispatch::apply_string;
use crate::layout::gsub::apply_ligature;
use crate::layout::matching::lookup_flags;
use crate::plan::feature_map::{FeatureMap, TableKind};
use crate::tables::arabic_presentation::{
    LIGATURES_2, LIGATURES_3, MARK_LIGATURES, PairLigatures, SHAPING, SHAPING_FIRST,
};

const FALLBACK_FEATURES: [Tag; 7] = [
    Tag::new(b"init"),
    Tag::new(b"medi"),
    Tag::new(b"fina"),
    Tag::new(b"isol"),
    Tag::new(b"rlig"),
    Tag::new(b"rlig"),
    Tag::new(b"rlig"),
];

type LigatureSets = Vec<(u16, Vec<(Vec<u16>, u16)>)>;

#[derive(Debug)]
enum Substitutions {
    Single(Vec<(u16, u16)>),
    Ligature(LigatureSets),
}

#[derive(Debug)]
struct FallbackLookup {
    mask: u32,
    filter: LookupFilter,
    substitutions: Substitutions,
}

#[derive(Debug)]
pub(crate) struct ArabicFallback {
    lookups: Vec<FallbackLookup>,
}

impl ArabicFallback {
    pub(crate) fn new(map: &FeatureMap, font: &FontContext) -> Option<Self> {
        let lookups: Vec<FallbackLookup> = FALLBACK_FEATURES
            .iter()
            .enumerate()
            .filter_map(|(index, &tag)| {
                let mask = map.one_mask(tag);
                if mask == 0 {
                    return None;
                }
                let (substitutions, props) = match index {
                    0..=3 => (
                        single_substitutions(font, index)?,
                        lookup_flags::IGNORE_MARKS,
                    ),
                    4 => (
                        ligature_substitutions(
                            font,
                            LIGATURES_3.iter().map(|(first, ligatures)| {
                                (
                                    *first,
                                    ligatures
                                        .iter()
                                        .map(|(rest, ligature)| (rest.to_vec(), *ligature))
                                        .collect(),
                                )
                            }),
                        )?,
                        lookup_flags::IGNORE_MARKS,
                    ),
                    5 => (
                        ligature_substitutions(font, pairs(LIGATURES_2))?,
                        lookup_flags::IGNORE_MARKS,
                    ),
                    _ => (ligature_substitutions(font, pairs(MARK_LIGATURES))?, 0),
                };
                Some(FallbackLookup {
                    mask,
                    filter: LookupFilter {
                        props,
                        reverse: false,
                        digest: substitutions.digest(),
                    },
                    substitutions,
                })
            })
            .collect();
        (!lookups.is_empty()).then_some(Self { lookups })
    }

    pub(crate) fn apply(&self, font: &FontContext, buffer: &mut Buffer) {
        let mut ctx = ApplyContext::new(TableKind::Gsub, font, buffer);
        for lookup in &self.lookups {
            ctx.set_lookup_mask(lookup.mask);
            apply_string(&mut ctx, &lookup.filter, None, |ctx| {
                lookup.substitutions.apply(ctx)
            });
        }
    }
}

fn pairs(table: &'static [PairLigatures]) -> impl Iterator<Item = (u16, Vec<(Vec<u16>, u16)>)> {
    table.iter().map(|(first, ligatures)| {
        (
            *first,
            ligatures
                .iter()
                .map(|(second, ligature)| (vec![*second], *ligature))
                .collect(),
        )
    })
}

fn single_substitutions(font: &FontContext, form: usize) -> Option<Substitutions> {
    let mut pairs: Vec<(u16, u16)> = SHAPING
        .iter()
        .enumerate()
        .filter_map(|(offset, forms)| {
            let presentation = forms[form];
            if presentation == 0 {
                return None;
            }
            let glyph = font.nominal_glyph(SHAPING_FIRST + offset as u32)?;
            let substitute = font.nominal_glyph(u32::from(presentation))?;
            (glyph != substitute).then_some((glyph, substitute))
        })
        .collect();
    if pairs.is_empty() {
        return None;
    }
    pairs.sort_by_key(|&(glyph, _)| glyph);
    Some(Substitutions::Single(pairs))
}

fn ligature_substitutions(
    font: &FontContext,
    table: impl Iterator<Item = (u16, Vec<(Vec<u16>, u16)>)>,
) -> Option<Substitutions> {
    let mut sets: LigatureSets = table
        .filter_map(|(first, ligatures)| {
            let first = font.nominal_glyph(u32::from(first))?;
            let ligatures: Vec<(Vec<u16>, u16)> = ligatures
                .into_iter()
                .filter_map(|(components, ligature)| {
                    let ligature = font.nominal_glyph(u32::from(ligature))?;
                    let components = components
                        .into_iter()
                        .map(|component| font.nominal_glyph(u32::from(component)))
                        .collect::<Option<Vec<u16>>>()?;
                    Some((components, ligature))
                })
                .collect();
            Some((first, ligatures))
        })
        .collect();
    sets.sort_by_key(|(first, _)| *first);
    if sets.iter().all(|(_, ligatures)| ligatures.is_empty()) {
        return None;
    }
    Some(Substitutions::Ligature(sets))
}

impl Substitutions {
    fn digest(&self) -> Digest {
        let mut digest = Digest::default();
        match self {
            Self::Single(pairs) => pairs
                .iter()
                .for_each(|(glyph, _)| digest.add(u32::from(*glyph))),
            Self::Ligature(sets) => sets
                .iter()
                .for_each(|(first, _)| digest.add(u32::from(*first))),
        }
        digest
    }

    fn apply(&self, ctx: &mut ApplyContext<'_>) -> bool {
        let glyph = ctx.buffer.cur(0).codepoint as u16;
        match self {
            Self::Single(pairs) => {
                let index = pairs.partition_point(|&(candidate, _)| candidate < glyph);
                match pairs.get(index) {
                    Some(&(candidate, substitute)) if candidate == glyph => {
                        ctx.replace_glyph(u32::from(substitute));
                        true
                    }
                    _ => false,
                }
            }
            Self::Ligature(sets) => {
                let index = sets.partition_point(|(candidate, _)| *candidate < glyph);
                let Some((candidate, ligatures)) = sets.get(index) else {
                    return false;
                };
                *candidate == glyph
                    && ligatures.iter().any(|(components, ligature)| {
                        apply_ligature(
                            ctx,
                            components.len(),
                            |position| components[position],
                            *ligature,
                        )
                    })
            }
        }
    }
}
