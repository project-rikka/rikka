use rikka_text_font::CoverageArray;

use crate::face::glyph_sets_builder::GlyphSetsBuilder;

#[derive(Debug, Default)]
pub(crate) struct CoverageSequence {
    chained: bool,
    backtrack: Box<[u16]>,
    input: Box<[u16]>,
    lookahead: Box<[u16]>,
}

impl CoverageSequence {
    pub(crate) fn build(
        chained: bool,
        [backtrack, input, lookahead]: [Option<CoverageArray<'_>>; 3],
        sets: &mut GlyphSetsBuilder,
    ) -> Option<Self> {
        Some(Self {
            chained,
            backtrack: Self::ids(backtrack, sets)?,
            input: Self::ids(input, sets)?,
            lookahead: Self::ids(lookahead, sets)?,
        })
    }

    fn ids(array: Option<CoverageArray<'_>>, sets: &mut GlyphSetsBuilder) -> Option<Box<[u16]>> {
        array
            .into_iter()
            .flat_map(|array| array.iter())
            .map(|coverage| match coverage {
                Some(coverage) if !coverage.is_ordered() => None,
                coverage => sets.add(coverage),
            })
            .collect()
    }

    #[inline]
    pub(crate) fn chained(&self) -> bool {
        self.chained
    }

    #[inline]
    pub(crate) fn backtrack(&self) -> &[u16] {
        &self.backtrack
    }

    #[inline]
    pub(crate) fn input(&self) -> &[u16] {
        &self.input
    }

    #[inline]
    pub(crate) fn lookahead(&self) -> &[u16] {
        &self.lookahead
    }
}
