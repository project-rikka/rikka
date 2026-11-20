use rikka_text_font::{
    ChainedSequenceContext, Coverage, Font, GlyphId, Gpos, Gsub, LayoutTable, PairPos, PosSubtable,
    SequenceContext, SubstSubtable, Tag,
};

fn glyph_for(font: &Font<'_>, codepoint: u32) -> GlyphId {
    let cmap = font.table(Tag::new(b"cmap")).expect("missing cmap table");
    let be16 = |at: usize| u16::from_be_bytes([cmap[at], cmap[at + 1]]);
    let be32 = |at: usize| u32::from_be_bytes([cmap[at], cmap[at + 1], cmap[at + 2], cmap[at + 3]]);
    for record in 0..usize::from(be16(2)) {
        let subtable = be32(4 + record * 8 + 4) as usize;
        match be16(subtable) {
            12 => {
                for group in 0..be32(subtable + 12) as usize {
                    let at = subtable + 16 + group * 12;
                    if (be32(at)..=be32(at + 4)).contains(&codepoint) {
                        return GlyphId((be32(at + 8) + codepoint - be32(at)) as u16);
                    }
                }
            }
            4 => {
                let segments = usize::from(be16(subtable + 6)) / 2;
                let ends = subtable + 14;
                let starts = ends + segments * 2 + 2;
                let deltas = starts + segments * 2;
                let ranges = deltas + segments * 2;
                for segment in 0..segments {
                    let (start, end) = (
                        u32::from(be16(starts + segment * 2)),
                        u32::from(be16(ends + segment * 2)),
                    );
                    if !(start..=end).contains(&codepoint) {
                        continue;
                    }
                    let delta = be16(deltas + segment * 2);
                    let range = usize::from(be16(ranges + segment * 2));
                    let glyph = if range == 0 {
                        (codepoint as u16).wrapping_add(delta)
                    } else {
                        let at = ranges + segment * 2 + range + (codepoint - start) as usize * 2;
                        match be16(at) {
                            0 => 0,
                            glyph => glyph.wrapping_add(delta),
                        }
                    };
                    return GlyphId(glyph);
                }
            }
            _ => {}
        }
    }
    panic!("U+{codepoint:04X} is not mapped")
}

fn feature_lookups(table: &LayoutTable<'_>, tag: &[u8; 4]) -> Vec<u16> {
    let mut lookups: Vec<u16> = table
        .features()
        .iter()
        .filter(|feature| feature.tag() == Tag::new(tag))
        .flat_map(|feature| feature.lookup_indices().iter())
        .collect();
    lookups.sort_unstable();
    lookups.dedup();
    lookups
}

fn subst_subtables<'a>(gsub: &Gsub<'a>, tag: &[u8; 4]) -> Vec<SubstSubtable<'a>> {
    feature_lookups(gsub, tag)
        .into_iter()
        .flat_map(|index| gsub.lookup(index).expect("missing lookup").subtables())
        .collect()
}

fn pos_subtables<'a>(gpos: &Gpos<'a>, tag: &[u8; 4]) -> Vec<PosSubtable<'a>> {
    feature_lookups(gpos, tag)
        .into_iter()
        .flat_map(|index| gpos.lookup(index).expect("missing lookup").subtables())
        .collect()
}

fn assert_coverage_within(coverage: Coverage<'_>, glyph_count: u16) {
    for glyph in coverage.glyphs() {
        assert!(glyph.0 < glyph_count, "covered glyph {glyph:?} exists");
        assert!(coverage.contains(glyph));
    }
}

#[test]
fn init_substitutes_isolated_arabic_letter() {
    let data = rikka_test_fonts::font_bytes("NotoSansArabic.ttf");
    let font = Font::new(&data).unwrap();
    let gsub = font.gsub().unwrap();
    let lam = glyph_for(&font, 0x0644);
    let initial = subst_subtables(&gsub, b"init")
        .into_iter()
        .find_map(|subtable| match subtable {
            SubstSubtable::Single(single) => single.substitute(lam),
            SubstSubtable::Multiple(multiple) => multiple.substitute(lam)?.first(),
            _ => None,
        })
        .expect("lam has no initial form");
    assert_ne!(initial, lam);
    assert!(initial.0 < font.glyph_count());
}

#[test]
fn cursive_attachment_connects_entry_and_exit() {
    let data = rikka_test_fonts::font_bytes("NotoNastaliqUrdu.ttf");
    let font = Font::new(&data).unwrap();
    let gpos = font.gpos().unwrap();
    let mut joined = 0;
    for subtable in pos_subtables(&gpos, b"curs") {
        let PosSubtable::Cursive(cursive) = subtable else {
            continue;
        };
        for glyph in cursive.coverage().glyphs() {
            let anchors = cursive
                .entry_exit(glyph)
                .expect("covered glyph has no record");
            if anchors.entry.is_some() && anchors.exit.is_some() {
                joined += 1;
            }
        }
    }
    assert!(joined > 20, "{joined} glyphs join on both sides");
}

#[test]
fn inter_kerns_av_pair() {
    let data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let gpos = font.gpos().unwrap();
    let gdef = font.gdef().unwrap();
    let store = gdef.var_store();
    let (a, v) = (glyph_for(&font, 'A' as u32), glyph_for(&font, 'V' as u32));
    let adjustment = pos_subtables(&gpos, b"kern")
        .into_iter()
        .find_map(|subtable| match subtable {
            PosSubtable::Pair(pairs) => pairs.values(a, v),
            _ => None,
        })
        .expect("no kerning for A V");
    let bold = font.location([(Tag::new(b"wght"), 900.0)]);
    let regular = adjustment.0.resolve(&[], store.as_ref());
    let heavy = adjustment.0.resolve(bold.coords(), store.as_ref());
    assert!(regular.x_advance < 0.0);
    assert!(heavy.x_advance < 0.0);
    assert_ne!(
        regular.x_advance, heavy.x_advance,
        "kerning varies with weight"
    );
}

#[test]
fn feature_variations_apply_inside_condition_range() {
    let data = rikka_test_fonts::font_bytes("InterVariable.ttf");
    let font = Font::new(&data).unwrap();
    let layout = font.gsub().unwrap();
    let Some(variations) = layout.feature_variations() else {
        return;
    };
    assert!(!variations.is_empty());
    let mut swapped = false;
    for weight in [100.0, 400.0, 700.0, 900.0] {
        let location = font.location([(Tag::new(b"wght"), weight)]);
        if let Some(record) = variations.find_index(location.coords()) {
            let substitution = variations
                .substitution(record)
                .expect("matched record has no feature substitution");
            for index in 0..layout.features().len() as u16 {
                let plain = layout.feature(index, None).unwrap();
                let varied = layout.feature(index, Some(record)).unwrap();
                assert_eq!(plain.tag(), varied.tag());
                if plain
                    .lookup_indices()
                    .iter()
                    .ne(varied.lookup_indices().iter())
                {
                    swapped = true;
                }
            }
            assert!(!substitution.is_empty());
        }
    }
    assert!(swapped);
}

#[test]
fn parses_every_lookup_subtable() {
    for name in [
        "NotoSansArabic.ttf",
        "NotoSansDevanagari.ttf",
        "InterVariable.ttf",
        "JetBrainsMono-Regular.ttf",
        "NotoSansMyanmar.ttf",
        "NotoSansKhmer.ttf",
    ] {
        let data = rikka_test_fonts::font_bytes(name);
        let font = Font::new(&data).unwrap();
        let glyphs = font.glyph_count();
        let gsub = font.gsub().unwrap();
        for index in 0..gsub.lookup_count() as u16 {
            let lookup = gsub.lookup(index).unwrap();
            assert!((1..=8).contains(&lookup.kind()), "{name} lookup {index}");
            assert_eq!(
                lookup.subtables().count(),
                lookup.subtable_count(),
                "{name} lookup {index}"
            );
            for subtable in lookup.subtables() {
                if let Some(coverage) = subtable.coverage() {
                    assert_coverage_within(coverage, glyphs);
                }
                if let SubstSubtable::Context(SequenceContext::Coverages(context)) = subtable {
                    assert!(context.input().iter().all(|coverage| coverage.is_some()));
                }
            }
        }
        let gpos = font.gpos().unwrap();
        for index in 0..gpos.lookup_count() as u16 {
            let lookup = gpos.lookup(index).unwrap();
            assert!((1..=8).contains(&lookup.kind()), "{name} lookup {index}");
            assert_eq!(
                lookup.subtables().count(),
                lookup.subtable_count(),
                "{name} lookup {index}"
            );
            for subtable in lookup.subtables() {
                assert_coverage_within(subtable.coverage().unwrap(), glyphs);
                if let PosSubtable::Pair(PairPos::Classes(pairs)) = subtable {
                    for (first, last, class) in pairs.first_class_def().ranges() {
                        assert!(first <= last && class < pairs.first_class_count());
                    }
                }
            }
        }
    }
}

fn walk_layout(table: &LayoutTable<'_>) -> usize {
    let mut touched = 0;
    let scripts = table.scripts();
    for index in 0..scripts.len() {
        if let Some(script) = scripts.get(index) {
            let systems =
                (0..script.lang_sys_count()).filter_map(|index| script.lang_sys_at(index));
            for lang_sys in script.default_lang_sys().into_iter().chain(systems) {
                touched += lang_sys
                    .feature_indices()
                    .iter()
                    .filter_map(|index| table.feature(index, Some(0)))
                    .count();
            }
        }
    }
    for feature in table.features().iter() {
        touched += feature.lookup_indices().len();
    }
    touched
}

fn walk_coverage(coverage: Option<Coverage<'_>>) -> usize {
    coverage.map_or(0, |coverage| coverage.ranges().take(64).count())
}

fn walk_gsub(gsub: &Gsub<'_>) -> usize {
    let mut touched = walk_layout(gsub);
    for index in 0..gsub.lookup_count() as u16 {
        let Some(lookup) = gsub.lookup(index) else {
            continue;
        };
        for subtable in lookup.subtables() {
            touched += walk_coverage(subtable.coverage());
            match subtable {
                SubstSubtable::Single(single) => {
                    touched += usize::from(single.substitute(GlyphId(5)).is_some())
                }
                SubstSubtable::Multiple(multiple) => {
                    touched += multiple.sequence(0).map_or(0, |s| s.len())
                }
                SubstSubtable::Alternate(alternate) => {
                    touched += alternate.alternate_set(0).map_or(0, |s| s.len())
                }
                SubstSubtable::Ligature(ligature) => {
                    touched += ligature
                        .ligature_set_at(0)
                        .map_or(0, |set| set.iter().count());
                }
                SubstSubtable::Context(context) => {
                    if let SequenceContext::Classes(classes) = context {
                        touched += classes.rule_set(1).map_or(0, |set| set.iter().count());
                    }
                }
                SubstSubtable::ChainContext(context) => {
                    if let ChainedSequenceContext::Classes(classes) = context {
                        touched += classes.rule_set(1).map_or(0, |set| set.iter().count());
                        touched += classes.input_class_def().ranges().take(64).count();
                    }
                }
                SubstSubtable::ReverseChainSingle(reverse) => {
                    touched += reverse.backtrack().iter().count()
                        + usize::from(reverse.substitute_at(0).is_some());
                }
            }
        }
    }
    touched
}

fn walk_gpos(gpos: &Gpos<'_>) -> usize {
    let mut touched = walk_layout(gpos);
    for index in 0..gpos.lookup_count() as u16 {
        let Some(lookup) = gpos.lookup(index) else {
            continue;
        };
        for subtable in lookup.subtables() {
            touched += walk_coverage(subtable.coverage());
            match subtable {
                PosSubtable::Single(single) => touched += usize::from(single.value_at(0).is_some()),
                PosSubtable::Pair(pairs) => {
                    if let PairPos::Glyphs(glyphs) = pairs
                        && let Some(set) = glyphs.pair_set(0)
                    {
                        touched += set.len();
                        touched += usize::from(set.find(GlyphId(30)).is_some());
                    }
                    touched += usize::from(pairs.values(GlyphId(30), GlyphId(40)).is_some());
                }
                PosSubtable::Cursive(cursive) => {
                    touched += usize::from(cursive.entry_exit_at(0).is_some())
                }
                PosSubtable::MarkBase(marks) => {
                    touched += usize::from(marks.marks().get(0).is_some());
                    touched += usize::from(marks.bases().anchor(0, 0).is_some());
                }
                PosSubtable::MarkLigature(marks) => {
                    touched += marks.ligature(0).map_or(0, |matrix| matrix.rows());
                }
                PosSubtable::MarkMark(marks) => {
                    touched += usize::from(marks.base_marks().anchor(0, 0).is_some())
                }
                PosSubtable::Context(_) | PosSubtable::ChainContext(_) => touched += 1,
            }
        }
    }
    touched
}

#[test]
fn corrupted_layout_tables_never_panic() {
    let data = rikka_test_fonts::font_bytes("NotoSansArabic.ttf");
    let font = Font::new(&data).unwrap();
    let gsub = font.table(Tag::new(b"GSUB")).unwrap();
    let gpos = font.table(Tag::new(b"GPOS")).unwrap();
    let gdef = font.table(Tag::new(b"GDEF")).unwrap();
    assert!(walk_gsub(&Gsub::new(gsub).unwrap()) > 0);
    assert!(walk_gpos(&Gpos::new(gpos).unwrap()) > 0);

    for table in [gsub, gpos, gdef] {
        let step = (table.len() / 97).max(1);
        for len in (0..table.len()).step_by(step) {
            let truncated = &table[..len];
            if let Some(gsub) = Gsub::new(truncated) {
                walk_gsub(&gsub);
            }
            if let Some(gpos) = Gpos::new(truncated) {
                walk_gpos(&gpos);
            }
            if let Some(gdef) = rikka_text_font::Gdef::new(truncated) {
                gdef.glyph_class(GlyphId(10));
                gdef.ligature_carets(GlyphId(10));
                gdef.var_store();
            }
        }
        let mut corrupted = table.to_vec();
        let mut state = 0x9E37_79B9_u32;
        for _ in 0..8 {
            for _ in 0..64 {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let at = state as usize % corrupted.len();
                corrupted[at] = (state >> 24) as u8;
            }
            if let Some(gsub) = Gsub::new(&corrupted) {
                walk_gsub(&gsub);
            }
            if let Some(gpos) = Gpos::new(&corrupted) {
                walk_gpos(&gpos);
            }
        }
    }
}

#[test]
fn kern_lookup_matches_linear_pair_scan() {
    let data = rikka_test_fonts::font_bytes("DejaVuSans.ttf");
    let font = Font::new(&data).unwrap();
    let kern = font.kern().expect("DejaVu Sans has no kern table");
    let raw = font.table(Tag::new(b"kern")).unwrap();
    let be16 = |at: usize| u16::from_be_bytes([raw[at], raw[at + 1]]);
    let pair_count = usize::from(be16(10));
    let mut checked = 0;
    for pair in (0..pair_count).step_by(7) {
        let at = 18 + pair * 6;
        let (left, right, value) = (
            GlyphId(be16(at)),
            GlyphId(be16(at + 2)),
            be16(at + 4) as i16,
        );
        assert_eq!(kern.horizontal_kerning(left, right), Some(i32::from(value)));
        checked += 1;
    }
    assert!(checked > 100);
    let (a, v) = (glyph_for(&font, 'A' as u32), glyph_for(&font, 'V' as u32));
    assert!(kern.horizontal_kerning(a, v).is_some_and(|value| value < 0));
    assert_eq!(kern.horizontal_kerning(GlyphId(0), GlyphId(0)), None);
}
