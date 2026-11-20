use rikka_text_font::{Coverage, GlyphId};

const PAGE_BITS: u32 = 8;
const PAGE_GLYPHS: usize = 1 << PAGE_BITS;
const NO_PAGE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) subtable: u16,
    pub(crate) coverage_index: u16,
}

#[derive(Debug, Default)]
pub(crate) struct Candidates {
    pages: Box<[u32]>,
    starts: Box<[u32]>,
    entries: Box<[Candidate]>,
}

impl Candidates {
    pub(crate) fn build(coverages: &[Option<Coverage<'_>>]) -> Self {
        let mut found: Vec<(u16, Candidate)> = Vec::new();
        for (subtable, coverage) in coverages.iter().enumerate() {
            let (Some(coverage), Ok(subtable)) = (coverage, u16::try_from(subtable)) else {
                continue;
            };
            let candidate = |(glyph, coverage_index): (GlyphId, u16)| {
                (
                    glyph.0,
                    Candidate {
                        subtable,
                        coverage_index,
                    },
                )
            };
            if coverage.is_ordered() {
                found.extend(coverage.entries().map(candidate));
            } else {
                found.extend(
                    coverage
                        .glyphs()
                        .filter_map(|glyph| Some((glyph, coverage.get(glyph)?)))
                        .map(candidate),
                );
            }
        }
        found.sort_by_key(|&(glyph, _)| glyph);
        found.dedup_by_key(|&mut (glyph, candidate)| (glyph, candidate.subtable));
        let page_count = found
            .last()
            .map_or(0, |&(glyph, _)| usize::from(glyph >> PAGE_BITS) + 1);
        let mut pages = vec![NO_PAGE; page_count];
        let mut starts: Vec<u32> = Vec::new();
        let mut cursor = 0;
        while cursor < found.len() {
            let page = usize::from(found[cursor].0 >> PAGE_BITS);
            pages[page] = (starts.len() / (PAGE_GLYPHS + 1)) as u32;
            let base = starts.len();
            starts.resize(base + PAGE_GLYPHS + 1, 0);
            for slot in 0..=PAGE_GLYPHS {
                let glyph = page * PAGE_GLYPHS + slot;
                while cursor < found.len() && usize::from(found[cursor].0) < glyph {
                    cursor += 1;
                }
                starts[base + slot] = cursor as u32;
            }
        }
        Self {
            pages: pages.into_boxed_slice(),
            starts: starts.into_boxed_slice(),
            entries: found.into_iter().map(|(_, candidate)| candidate).collect(),
        }
    }

    #[inline]
    pub(crate) fn get(&self, glyph: u32) -> &[Candidate] {
        let Some(&page) = self.pages.get((glyph >> PAGE_BITS) as usize) else {
            return &[];
        };
        if page == NO_PAGE {
            return &[];
        }
        let slot = page as usize * (PAGE_GLYPHS + 1) + glyph as usize % PAGE_GLYPHS;
        let (start, end) = (self.starts[slot], self.starts[slot + 1]);
        &self.entries[start as usize..end as usize]
    }

    #[inline]
    pub(crate) fn covers(&self, glyph: u32) -> bool {
        !self.get(glyph).is_empty()
    }
}
