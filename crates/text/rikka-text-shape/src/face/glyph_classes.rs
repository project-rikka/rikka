use rikka_text_font::Gdef;

use crate::buffer::glyph_info::glyph_props;
use crate::face::glyph_sets::GlyphSets;
use crate::face::glyph_sets_builder::GlyphSetsBuilder;

#[derive(Debug, Default)]
pub(crate) struct GlyphClasses {
    props: Box<[u16]>,
    mark_sets: Box<[u16]>,
    mark_members: GlyphSets,
}

impl GlyphClasses {
    pub(crate) fn new(gdef: Option<&Gdef<'_>>, glyph_count: u16) -> Self {
        let Some(gdef) = gdef else {
            return Self::default();
        };
        let classes = gdef.glyph_class_def();
        let attach_classes = gdef.mark_attach_class_def();
        let covered_end = classes
            .ranges()
            .map(|(_, last, _)| usize::from(last.0) + 1)
            .max()
            .unwrap_or(0);
        let mut props = vec![0u16; covered_end.max(usize::from(glyph_count))];
        for (first, last, class) in classes.ranges() {
            let value = match class {
                1 => glyph_props::BASE_GLYPH,
                2 => glyph_props::LIGATURE,
                3 => glyph_props::MARK,
                _ => 0,
            };
            for slot in &mut props[usize::from(first.0)..=usize::from(last.0)] {
                *slot = value;
            }
        }
        for (first, last, class) in attach_classes.ranges() {
            for glyph in usize::from(first.0)..=usize::from(last.0) {
                if let Some(slot) = props.get_mut(glyph)
                    && *slot == glyph_props::MARK
                {
                    *slot |= class << 8;
                }
            }
        }
        let mut members = GlyphSetsBuilder::default();
        let mark_sets = gdef
            .mark_glyph_sets()
            .map(|sets| {
                (0..sets.len())
                    .map_while(|set| members.add(sets.get(set as u16)))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            props: props.into_boxed_slice(),
            mark_sets,
            mark_members: members.build(),
        }
    }

    #[inline]
    pub(crate) fn props(&self, glyph: u32) -> u16 {
        self.props.get(glyph as usize).copied().unwrap_or(0)
    }

    #[inline]
    pub(crate) fn is_mark_in_set(&self, glyph: u32, set: u16) -> bool {
        self.mark_sets
            .get(usize::from(set))
            .is_some_and(|&members| self.mark_members.contains(members, glyph))
    }
}
