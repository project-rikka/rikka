use crate::support::{Mutator, Path};
use rikka_test_fonts::Writer;
use rikka_text_font::{Error, Font, FontFile, GlyphId, Location, NameId, OutlineScratch, Tag};

fn exercise(font: &Font<'_>, mutator: &mut Mutator, scratch: &mut OutlineScratch) {
    let location = font.location([
        (Tag::new(b"wght"), 700.0),
        (Tag::new(b"opsz"), 20.0),
        (Tag::new(b"FILL"), 1.0),
    ]);
    let _ = font.named_instance_location(0);
    let _ = font.metrics(&location);
    let metrics = font.glyph_metrics(&location);
    let glyph_count = usize::from(font.glyph_count()) + 2;
    for _ in 0..24 {
        let glyph = GlyphId(mutator.below(glyph_count) as u16);
        let _ = metrics.advance_width(glyph);
        let _ = metrics.left_side_bearing(glyph);
        let _ = metrics.advance_height(glyph);
        for at in [&Location::default(), &location] {
            let mut path = Path::default();
            let _ = font.outline(glyph, at, scratch, &mut path);
            let _ = font.glyph_bounds(glyph, at, scratch);
        }
        let _ = font.post().and_then(|post| post.glyph_name(glyph));
    }
    if let Some(charmap) = font.charmap() {
        for _ in 0..64 {
            let codepoint = mutator.next() as u32 % 0x11_0000;
            let _ = charmap.map(codepoint);
            let _ = charmap.map_variant(codepoint, 0xFE0F);
        }
        let mut mappings = 0;
        charmap.for_each_mapping(|_, _| mappings += 1);
    }
    if let Some(name) = font.name() {
        let _ = name.get(NameId::FAMILY);
        let _ = name.localized(NameId::FULL_NAME, "ja");
    }
    if let Some(stat) = font.stat() {
        let _ = stat.design_axes().count();
        let _ = stat.axis_values().count();
    }
    if let Some(fvar) = font.fvar() {
        let _ = fvar
            .instances()
            .map(|instance| instance.coordinates().count())
            .sum::<usize>();
    }
}

fn corrupt_tables(file: &str, tables: &[&[u8; 4]], rounds: u64) {
    let original = rikka_test_fonts::font_bytes(file);
    let mut scratch = OutlineScratch::new();
    for round in 0..rounds {
        let mut mutator = Mutator::new(round * 7919 + file.len() as u64);
        let mut data = original.clone();
        let tag = tables[mutator.below(tables.len())];
        if let Some(range) = crate::support::table_range(&data, 0, tag)
            .filter(|range| !range.is_empty() && range.end <= data.len())
        {
            let flips = 1 + mutator.below(16);
            mutator.corrupt(&mut data, range, flips);
        }
        let Ok(file) = FontFile::new(&data) else {
            continue;
        };
        let Ok(font) = file.face(0) else { continue };
        exercise(&font, &mut mutator, &mut scratch);
    }
}

#[test]
fn corrupted_truetype_tables_never_panic() {
    corrupt_tables(
        "InterVariable.ttf",
        &[
            b"glyf", b"loca", b"gvar", b"hmtx", b"hhea", b"HVAR", b"MVAR", b"cmap", b"name",
            b"post", b"fvar", b"avar", b"STAT", b"OS/2", b"head", b"maxp",
        ],
        400,
    );
    corrupt_tables(
        "MaterialSymbolsSharp[FILL,GRAD,opsz,wght].ttf",
        &[b"glyf", b"loca", b"gvar", b"HVAR", b"fvar", b"avar"],
        150,
    );
}

#[test]
fn corrupted_auxiliary_tables_never_panic() {
    corrupt_tables(
        "NotoSansDevanagari.ttf",
        &[b"glyf", b"loca", b"gvar", b"cmap", b"avar", b"fvar"],
        150,
    );
    corrupt_tables(
        "Font Awesome 7 Free-Solid-900.otf",
        &[b"CFF ", b"cmap", b"hmtx", b"name", b"post"],
        150,
    );
}

#[test]
fn truncated_files_never_panic() {
    let original = rikka_test_fonts::font_bytes("JetBrainsMono-Regular.ttf");
    let mut scratch = OutlineScratch::new();
    let mut mutator = Mutator::new(3);
    for step in 1..=200 {
        let len = original.len() * step / 200;
        let data = &original[..len];
        let Ok(file) = FontFile::new(data) else {
            continue;
        };
        let Ok(font) = file.face(0) else { continue };
        exercise(&font, &mut mutator, &mut scratch);
    }
}

#[test]
fn deeply_nested_composites_are_cut_off() {
    let glyph_count = 40u16;
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for glyph in 0..glyph_count {
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        let mut record = Writer::new();
        record.i16(-1).i16(0).i16(0).i16(100).i16(100);
        let child = (glyph + 1) % glyph_count;
        record.u16(0x0023).u16(child).i16(1).i16(1);
        record.u16(0x0003).u16(child).i16(2).i16(2);
        glyf.extend_from_slice(&record.finish());
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
    let mut head = vec![0u8; 54];
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    head[50..52].copy_from_slice(&1i16.to_be_bytes());
    let data = crate::support::sfnt(
        glyph_count,
        &[(b"head", head), (b"glyf", glyf), (b"loca", loca)],
    );
    let font = Font::new(&data).unwrap();
    let mut path = Path::default();
    assert_eq!(
        font.outline(
            GlyphId(0),
            &Location::default(),
            &mut OutlineScratch::new(),
            &mut path
        ),
        Err(Error::Malformed)
    );
}
