use rikka_test_fonts::Writer;
use rikka_text_font::{
    BitmapData, BitmapStrikes, Bounds, Brush, ColorPainter, Colr, CompositeMode, Cpal, Error,
    F2Dot14, Font, GlyphId, SbixFormat, Transform,
};

const GLYPH_COUNT: u16 = 30;

fn cpal_table() -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(1).u16(3).u16(2).u16(6);
    let records_at = w.len();
    w.u32(0);
    w.u16(0).u16(3);
    let types_at = w.len();
    w.u32(0);
    let labels_at = w.len();
    w.u32(0);
    let entry_labels_at = w.len();
    w.u32(0);
    let records = w.len();
    w.set_u32(records_at, records);
    for [r, g, b, a] in [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 128],
        [10, 20, 30, 255],
        [40, 50, 60, 255],
        [70, 80, 90, 0],
    ] {
        w.u8(b).u8(g).u8(r).u8(a);
    }
    let types = w.len();
    w.set_u32(types_at, types);
    w.u32(1).u32(2);
    let labels = w.len();
    w.set_u32(labels_at, labels);
    w.u16(300).u16(0xFFFF);
    let entry_labels = w.len();
    w.set_u32(entry_labels_at, entry_labels);
    w.u16(400).u16(401).u16(0xFFFF);
    w.finish()
}

struct PaintBuilder {
    w: Writer,
    pending: Vec<(usize, usize, &'static str)>,
    named: Vec<(&'static str, usize)>,
}

impl PaintBuilder {
    fn name(&mut self, name: &'static str) -> &mut Writer {
        self.named.push((name, self.w.len()));
        &mut self.w
    }

    fn child(&mut self, paint: usize, target: &'static str) {
        let at = self.w.len();
        self.w.u24(0);
        self.pending.push((at, paint, target));
    }

    fn resolve(&mut self) {
        for (at, base, target) in std::mem::take(&mut self.pending) {
            let target = self
                .named
                .iter()
                .find(|(name, _)| *name == target)
                .unwrap()
                .1;
            self.w.set_u24(at, target - base);
        }
    }

    fn at(&self, name: &str) -> usize {
        self.named
            .iter()
            .find(|(known, _)| *known == name)
            .unwrap()
            .1
    }
}

fn colr_table() -> Vec<u8> {
    let mut b = PaintBuilder {
        w: Writer::new(),
        pending: Vec::new(),
        named: Vec::new(),
    };
    b.w.u16(1)
        .u16(1)
        .u32(0)
        .u32(0)
        .u16(2)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0);
    let base_records = b.w.len();
    b.w.set_u32(4, base_records);
    b.w.u16(5).u16(0).u16(2);
    let layer_records = b.w.len();
    b.w.set_u32(8, layer_records);
    b.w.u16(20).u16(1).u16(21).u16(0xFFFF);

    let base_glyphs: [(u16, &str); 8] = [
        (1, "p1"),
        (2, "p2"),
        (3, "p3"),
        (4, "p4"),
        (6, "p6"),
        (7, "p7"),
        (8, "p8"),
        (9, "p9"),
    ];
    let base_list = b.w.len();
    b.w.set_u32(14, base_list);
    b.w.u32(base_glyphs.len() as u32);
    let mut base_offsets = Vec::new();
    for (glyph, name) in base_glyphs {
        b.w.u16(glyph);
        base_offsets.push((b.w.len(), name));
        b.w.u32(0);
    }
    let layer_list = b.w.len();
    b.w.set_u32(18, layer_list);
    b.w.u32(2);
    let layer_offsets = [(b.w.len(), "l0"), (b.w.len() + 4, "l1")];
    b.w.u32(0).u32(0);

    b.name("p1").u8(1).u8(2).u32(0);
    b.name("l0").u8(10);
    let at = b.at("l0");
    b.child(at, "s0");
    b.w.u16(10);
    b.name("s0").u8(2).u16(0).i16(0x2000);
    b.name("l1").u8(15);
    let at = b.at("l1");
    b.child(at, "g1");
    b.w.i16(100).i16(0).u32(6);
    b.name("g1").u8(10);
    let at = b.at("g1");
    b.child(at, "lg");
    b.w.u16(11);
    b.name("lg").u8(5);
    let at = b.at("lg");
    b.child(at, "vcl");
    b.w.i16(0).i16(0).i16(200).i16(0).i16(0).i16(200).u32(0);
    b.name("vcl").u8(1).u16(2);
    b.w.i16(0).u16(1).i16(0x4000).u32(8);
    b.w.i16(0x4000).u16(2).i16(0x4000).u32(0xFFFF_FFFF);
    b.name("p2").u8(11).u16(3);
    b.name("p3").u8(11).u16(2);
    b.name("p4").u8(32);
    let at = b.at("p4");
    b.child(at, "rot");
    b.w.u8(23);
    b.child(at, "fg");
    b.name("rot").u8(26);
    let at = b.at("rot");
    b.child(at, "fg");
    b.w.i16(0x2000).i16(50).i16(50);
    b.name("p9").u8(12);
    let at = b.at("p9");
    b.child(at, "fg");
    b.child(at, "affine");
    b.name("affine")
        .u32(0x2_0000)
        .u32(0)
        .u32(0)
        .u32(0x8000)
        .u32(0xA_0000)
        .u32((-3i32 * 65536) as u32);
    b.name("fg").u8(2).u16(0xFFFF).i16(0x4000);
    b.name("p6").u8(11).u16(1);
    b.name("p7").u8(8);
    let at = b.at("p7");
    b.child(at, "cl");
    b.w.i16(10).i16(20).i16(-0x4000).i16(0x4000);
    b.name("p8").u8(6);
    let at = b.at("p8");
    b.child(at, "cl");
    b.w.i16(1).i16(2).u16(3).i16(4).i16(5).u16(6);
    b.name("cl").u8(0).u16(1).i16(0).u16(2).i16(0x4000);
    b.resolve();
    for (at, name) in base_offsets {
        let target = b.at(name);
        b.w.set_u32(at, target - base_list);
    }
    for (at, name) in layer_offsets {
        let target = b.at(name);
        b.w.set_u32(at, target - layer_list);
    }

    let clip_list = b.w.len();
    b.w.set_u32(22, clip_list);
    b.w.u8(1).u32(1).u16(1).u16(1);
    let clip_offset_at = b.w.len();
    b.w.u24(0);
    let clip_box = b.w.len();
    b.w.set_u24(clip_offset_at, clip_box - clip_list);
    b.w.u8(2).i16(0).i16(-10).i16(300).i16(290).u32(10);

    let store = b.w.len();
    b.w.set_u32(30, store);
    b.w.u16(1).u32(12).u16(1).u32(22);
    b.w.u16(1).u16(1).i16(0).i16(0x4000).i16(0x4000);
    let deltas: [i8; 14] = [0, 0, 50, 0, 0, 0, 20, -10, 0, -64, -5, 0, 5, 0];
    b.w.u16(deltas.len() as u16).u16(0).u16(1).u16(0);
    for delta in deltas {
        b.w.u8(delta as u8);
    }
    b.w.finish()
}

#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
    depth: i32,
}

fn round(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0 + 0.0
}

fn show(bounds: Bounds) -> String {
    format!(
        "{} {} {} {}",
        round(bounds.x_min),
        round(bounds.y_min),
        round(bounds.x_max),
        round(bounds.y_max)
    )
}

impl ColorPainter for Recorder {
    fn push_transform(&mut self, t: Transform) {
        self.depth += 1;
        self.calls.push(format!(
            "transform {} {} {} {} {} {}",
            round(t.xx),
            round(t.yx),
            round(t.xy),
            round(t.yy),
            round(t.dx),
            round(t.dy)
        ));
    }

    fn pop_transform(&mut self) {
        self.depth -= 1;
        self.calls.push("pop transform".into());
    }

    fn push_clip_glyph(&mut self, glyph: GlyphId) {
        self.depth += 1;
        self.calls.push(format!("clip glyph {}", glyph.0));
    }

    fn push_clip_box(&mut self, clip: Bounds) {
        self.depth += 1;
        self.calls.push(format!("clip box {}", show(clip)));
    }

    fn pop_clip(&mut self) {
        self.depth -= 1;
        self.calls.push("pop clip".into());
    }

    fn fill(&mut self, brush: Brush<'_>) {
        let stops = |stops: rikka_text_font::ColorStops<'_>| {
            stops
                .iter()
                .map(|stop| {
                    format!(
                        "{}:{:?}:{}",
                        round(stop.offset),
                        stop.color,
                        round(stop.alpha)
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let call = match brush {
            Brush::Solid { color, alpha } => format!("solid {color:?} {}", round(alpha)),
            Brush::LinearGradient {
                start,
                end,
                stops: s,
                extend,
            } => format!(
                "linear {} {} {} {} {extend:?} [{}]",
                round(start.x),
                round(start.y),
                round(end.x),
                round(end.y),
                stops(s)
            ),
            Brush::RadialGradient {
                start_center,
                start_radius,
                end_center,
                end_radius,
                stops: s,
                extend,
            } => format!(
                "radial {} {} {} {} {} {} {extend:?} [{}]",
                start_center.x,
                start_center.y,
                start_radius,
                end_center.x,
                end_center.y,
                end_radius,
                stops(s)
            ),
            Brush::SweepGradient {
                center,
                start_degrees,
                end_degrees,
                stops: s,
                extend,
            } => format!(
                "sweep {} {} {} {} {extend:?} [{}]",
                center.x,
                center.y,
                round(start_degrees),
                round(end_degrees),
                stops(s)
            ),
        };
        self.calls.push(call);
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        self.depth += 1;
        self.calls.push(format!("layer {mode:?}"));
    }

    fn pop_layer(&mut self) {
        self.depth -= 1;
        self.calls.push("pop layer".into());
    }
}

fn color_font() -> Vec<u8> {
    crate::support::sfnt(
        GLYPH_COUNT,
        &[(b"COLR", colr_table()), (b"CPAL", cpal_table())],
    )
}

fn paint(font: &Font<'_>, glyph: u16, coords: &[F2Dot14]) -> (Result<(), Error>, Recorder) {
    let mut recorder = Recorder::default();
    let result = font
        .color_glyph(GlyphId(glyph))
        .unwrap()
        .paint(coords, &mut recorder);
    assert_eq!(recorder.depth, 0, "every push is popped");
    (result, recorder)
}

const LAYERED: [&str; 9] = [
    "clip glyph 10",
    "solid Palette(0) 0.5",
    "pop clip",
    "transform 1 0 0 1 100 0",
    "clip glyph 11",
    "linear 0 0 200 0 Repeat [0:Palette(1):1,1:Palette(2):1]",
    "pop clip",
    "pop transform",
    "pop clip",
];

#[test]
fn colr_v1_paints_layers_transforms_and_gradients() {
    let data = color_font();
    let font = Font::new(&data).unwrap();
    let (result, recorder) = paint(&font, 1, &[]);
    result.unwrap();
    let mut expected = vec!["clip box 0 -10 300 290"];
    expected.extend(&LAYERED[..9]);
    assert_eq!(recorder.calls, expected);
}

#[test]
fn paint_cycle_is_reported_and_balanced() {
    let data = color_font();
    let font = Font::new(&data).unwrap();
    let (result, _) = paint(&font, 2, &[]);
    assert_eq!(result, Err(Error::PaintCycle));
}

#[derive(Default)]
struct Counter(usize);

impl ColorPainter for Counter {
    fn push_transform(&mut self, _: Transform) {
        self.0 += 1;
    }
    fn pop_transform(&mut self) {}
    fn push_clip_glyph(&mut self, _: GlyphId) {
        self.0 += 1;
    }
    fn push_clip_box(&mut self, _: Bounds) {
        self.0 += 1;
    }
    fn pop_clip(&mut self) {}
    fn fill(&mut self, brush: Brush<'_>) {
        if let Brush::LinearGradient { stops, .. }
        | Brush::RadialGradient { stops, .. }
        | Brush::SweepGradient { stops, .. } = brush
        {
            self.0 += stops.iter().count();
        }
    }
    fn push_layer(&mut self, _: CompositeMode) {
        self.0 += 1;
    }
    fn pop_layer(&mut self) {}
}

#[test]
fn truncated_color_tables_never_panic() {
    let colr = colr_table();
    let cpal = cpal_table();
    for len in 0..colr.len() {
        let Some(table) = Colr::new(&colr[..len]) else {
            continue;
        };
        for glyph in 0..12 {
            if let Some(color_glyph) = table.color_glyph(GlyphId(glyph)) {
                let _ = color_glyph.paint(&[F2Dot14::ONE], &mut Counter::default());
                let _ = color_glyph.clip_box(&[F2Dot14::ONE]);
            }
        }
    }
    for len in 0..cpal.len() {
        if let Some(table) = Cpal::new(&cpal[..len]) {
            for palette in table.palettes() {
                let _ = palette.colors().count();
            }
        }
    }
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[test]
fn noto_color_emoji_pngs_match_metrics() {
    let data = rikka_test_fonts::font_bytes("NotoColorEmoji.ttf");
    let font = Font::new(&data).unwrap();
    let strikes = font.bitmap_strikes().unwrap();
    assert!(!strikes.is_empty());
    let strike = strikes.get(0).unwrap();
    assert_eq!(strike.bit_depth(), 32);
    assert_eq!(strike.ppem_y(), 109);
    let mut images = 0;
    for glyph in strike.first_glyph().0..=strike.last_glyph().0 {
        let Some(bitmap) = strike.glyph(GlyphId(glyph)) else {
            continue;
        };
        let BitmapData::Png(png) = bitmap.data else {
            panic!("glyph {glyph} is not a PNG");
        };
        assert!(png.starts_with(&PNG_SIGNATURE));
        assert_eq!(&png[12..16], b"IHDR");
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(
            (width, height),
            (
                u32::from(bitmap.metrics.width),
                u32::from(bitmap.metrics.height)
            )
        );
        assert!(bitmap.metrics.advance > 0);
        images += 1;
    }
    assert!(images > 3000, "{images} emoji images");
    let first = font
        .bitmap_glyph(GlyphId(strike.first_glyph().0 + 10), 16)
        .unwrap();
    assert_eq!(first.ppem_y, 109);
    assert!(font.color_glyph(GlyphId(10)).is_none());
}

fn index_subtable(w: &mut Writer, index_format: u16, image_format: u16, image_offset: u32) {
    w.u16(index_format).u16(image_format).u32(image_offset);
}

fn big_metrics(w: &mut Writer, width: u8, height: u8) {
    w.u8(height)
        .u8(width)
        .u8(1)
        .u8(height)
        .u8(width + 1)
        .u8(0)
        .u8(0)
        .u8(0);
}

fn bitmap_tables() -> (Vec<u8>, Vec<u8>) {
    let mut images = Writer::new();
    images.u16(3).u16(0);
    let mut subtables: Vec<(u16, u16, Writer)> = Vec::new();

    let base = images.len() as u32;
    images
        .u8(2)
        .u8(3)
        .u8(0)
        .u8(2)
        .u8(4)
        .u8(0b1010_0000)
        .u8(0b0100_0000);
    images.u8(1).u8(1).u8(0).u8(1).u8(2).u8(0b1000_0000);
    images
        .u8(1)
        .u8(1)
        .u8(0)
        .u8(1)
        .u8(2)
        .u8(0)
        .u16(2)
        .u16(1)
        .u8(0)
        .u8(0)
        .u16(3)
        .u8(2)
        .u8(0xFF);
    let mut s = Writer::new();
    index_subtable(&mut s, 1, 1, base);
    s.u32(0).u32(7).u32(13);
    subtables.push((1, 2, s));
    let mut s = Writer::new();
    index_subtable(&mut s, 1, 8, base + 13);
    s.u32(0).u32(16);
    subtables.push((6, 6, s));

    let base = images.len() as u32;
    images.u8(0b1001_0110).u8(0b0110_1001);
    let mut s = Writer::new();
    index_subtable(&mut s, 2, 5, base);
    s.u32(2);
    big_metrics(&mut s, 4, 4);
    subtables.push((3, 3, s));

    let base = images.len() as u32;
    big_metrics(&mut images, 3, 3);
    images.u8(0b1110_0011).u8(0b1000_0000);
    let mut s = Writer::new();
    index_subtable(&mut s, 3, 7, base);
    s.u16(0).u16(10);
    subtables.push((4, 4, s));

    let base = images.len() as u32;
    big_metrics(&mut images, 2, 2);
    images.u8(0xC0).u8(0x40);
    big_metrics(&mut images, 2, 2);
    images.u8(0x80).u8(0x80);
    let mut s = Writer::new();
    index_subtable(&mut s, 4, 6, base);
    s.u32(2).u16(5).u16(0).u16(7).u16(10).u16(0).u16(20);
    subtables.push((5, 7, s));

    let base = images.len() as u32;
    images.u8(0b1011_0000).u8(0b0100_0000);
    let mut s = Writer::new();
    index_subtable(&mut s, 5, 5, base);
    s.u32(1);
    big_metrics(&mut s, 2, 2);
    s.u32(2).u16(8).u16(9);
    subtables.push((8, 9, s));

    let png_glyph1 = images.len() as u32;
    images
        .u8(4)
        .u8(4)
        .u8(0)
        .u8(4)
        .u8(5)
        .u32(10)
        .raw(&PNG_SIGNATURE)
        .u16(0xABCD);
    let mut s = Writer::new();
    index_subtable(&mut s, 1, 17, png_glyph1);
    s.u32(0).u32(19);
    let large_subtables = vec![(1u16, 1u16, s)];
    let png_glyph2 = images.len() as u32;
    images.u32(9).raw(&PNG_SIGNATURE).u8(1);
    let mut s = Writer::new();
    index_subtable(&mut s, 2, 19, png_glyph2);
    s.u32(13);
    big_metrics(&mut s, 6, 6);
    let large_subtables: Vec<_> = large_subtables
        .into_iter()
        .chain([(2u16, 2u16, s)])
        .collect();

    let mut location = Writer::new();
    location.u16(3).u16(0).u32(2);
    let strike_records = location.len();
    location.raw(&[0; 96]);
    for (strike, (subtables, ppem, depth, first, last)) in [
        (&subtables, 12u8, 1u8, 1u16, 9u16),
        (&large_subtables, 20, 32, 1, 2),
    ]
    .into_iter()
    .enumerate()
    {
        let list = location.len();
        let record = strike_records + strike * 48;
        location.set_u32(record, list);
        location.set_u32(record + 8, subtables.len());
        location.set_u16(record + 40, first as usize);
        location.set_u16(record + 42, last as usize);
        location.set_u8(record + 44, ppem);
        location.set_u8(record + 45, ppem);
        location.set_u8(record + 46, depth);
        let mut offset = subtables.len() * 8;
        for (first, last, subtable) in subtables {
            location.u16(*first).u16(*last).u32(offset as u32);
            offset += subtable.len();
        }
        for (_, _, subtable) in subtables {
            location.raw(subtable.bytes());
        }
        let size = location.len() - list;
        location.set_u32(record + 4, size);
    }
    (location.finish(), images.finish())
}

#[test]
fn decodes_every_bitmap_index_and_image_format() {
    let (location, images) = bitmap_tables();
    let data = crate::support::sfnt(GLYPH_COUNT, &[(b"CBLC", location), (b"CBDT", images)]);
    let font = Font::new(&data).unwrap();
    let strikes = font.bitmap_strikes().unwrap();
    assert_eq!(strikes.len(), 2);
    let small = strikes.get(0).unwrap();
    let mask = |glyph: u16| {
        let bitmap = small.glyph(GlyphId(glyph)).unwrap();
        let BitmapData::Mask { data, bit_aligned } = bitmap.data else {
            panic!("glyph {glyph} is not a mask");
        };
        (
            bitmap.metrics.width,
            bitmap.metrics.height,
            data.to_vec(),
            bit_aligned,
        )
    };
    assert_eq!(mask(1), (3, 2, vec![0b1010_0000, 0b0100_0000], false));
    assert_eq!(mask(2), (1, 1, vec![0b1000_0000], false));
    assert_eq!(mask(3), (4, 4, vec![0b1001_0110, 0b0110_1001], true));
    assert_eq!(mask(4), (3, 3, vec![0b1110_0011, 0b1000_0000], true));
    assert_eq!(mask(5), (2, 2, vec![0xC0, 0x40], false));
    assert_eq!(mask(7), (2, 2, vec![0x80, 0x80], false));
    assert_eq!(mask(8), (2, 2, vec![0b1011_0000], true));
    assert_eq!(mask(9), (2, 2, vec![0b0100_0000], true));
    assert!(small.glyph(GlyphId(6)).is_some());
    let BitmapData::Composite(components) = small.glyph(GlyphId(6)).unwrap().data else {
        panic!("glyph 6 is not a composite");
    };
    let components: Vec<_> = components
        .iter()
        .map(|c| (c.glyph.0, c.x_offset, c.y_offset))
        .collect();
    assert_eq!(components, [(1, 0, 0), (3, 2, -1)]);
    assert!(small.glyph(GlyphId(10)).is_none());

    let large = font.bitmap_glyph(GlyphId(1), 16).unwrap();
    assert_eq!((large.ppem_y, large.bit_depth), (20, 32));
    assert!(
        matches!(large.data, BitmapData::Png(png) if png.len() == 10 && png.starts_with(&PNG_SIGNATURE))
    );
    let indexed = font.bitmap_glyph(GlyphId(2), 20).unwrap();
    assert_eq!((indexed.metrics.width, indexed.metrics.advance), (6, 7));
    assert!(matches!(indexed.data, BitmapData::Png(png) if png.len() == 9));
    assert_eq!(font.bitmap_glyph(GlyphId(1), 12).unwrap().ppem_y, 12);
    assert_eq!(font.bitmap_glyph(GlyphId(1), 8).unwrap().ppem_y, 12);
    assert_eq!(font.bitmap_glyph(GlyphId(1), 40).unwrap().ppem_y, 20);
    assert_eq!(font.bitmap_glyph(GlyphId(3), 16).unwrap().ppem_y, 12);
}

#[test]
fn truncated_synthetic_bitmap_tables_never_panic() {
    let (location, images) = bitmap_tables();
    for len in 0..location.len() {
        if let Some(strikes) = BitmapStrikes::new(&location[..len], &images) {
            for glyph in 0..12 {
                let _ = strikes.glyph(GlyphId(glyph), 12);
            }
        }
    }
    for len in 0..images.len() {
        let strikes = BitmapStrikes::new(&location, &images[..len]).unwrap();
        for glyph in 0..12 {
            let _ = strikes.glyph(GlyphId(glyph), 12);
        }
    }
}

fn sbix_table() -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(1).u16(0x0003).u32(2).u32(0).u32(0);
    let strike = |w: &mut Writer, ppem: u16, glyphs: &[(u16, &[u8; 4], &[u8])]| {
        let start = w.len();
        w.u16(ppem).u16(72);
        let offsets = w.len();
        w.raw(&vec![0; (usize::from(GLYPH_COUNT) + 1) * 4]);
        let mut cursor = w.len() - start;
        for glyph in 0..=GLYPH_COUNT {
            w.set_u32(offsets + usize::from(glyph) * 4, cursor);
            if let Some((_, kind, data)) = glyphs.iter().find(|(id, _, _)| *id == glyph) {
                w.i16(-2).i16(3).raw(*kind).raw(data);
                cursor = w.len() - start;
            }
        }
        start
    };
    let first = strike(
        &mut w,
        32,
        &[
            (1, b"png ", &PNG_SIGNATURE),
            (2, b"dupe", &[0, 1]),
            (3, b"dupe", &[0, 4]),
            (4, b"dupe", &[0, 3]),
            (5, b"mask", &[1, 2, 3]),
        ],
    );
    w.set_u32(8, first);
    let second = strike(&mut w, 64, &[(1, b"jpg ", &[0xFF, 0xD8])]);
    w.set_u32(12, second);
    w.finish()
}

#[test]
fn sbix_resolves_dupe_glyphs() {
    let data = crate::support::sfnt(GLYPH_COUNT, &[(b"sbix", sbix_table())]);
    let font = Font::new(&data).unwrap();
    let sbix = font.sbix().unwrap();
    assert!(sbix.draws_outlines());
    assert_eq!(sbix.strike_count(), 2);
    let strike = sbix.strike(0).unwrap();
    assert_eq!((strike.ppem(), strike.ppi()), (32, 72));
    let png = strike.glyph(GlyphId(1)).unwrap();
    assert_eq!(
        (png.origin_x, png.origin_y, png.format),
        (-2, 3, SbixFormat::Png)
    );
    assert_eq!(png.data, PNG_SIGNATURE);
    let dupe = strike.glyph(GlyphId(2)).unwrap();
    assert_eq!(
        (dupe.format, dupe.data),
        (SbixFormat::Png, &PNG_SIGNATURE[..])
    );
    assert!(strike.glyph(GlyphId(3)).is_none());
    assert_eq!(strike.glyph(GlyphId(5)).unwrap().format, SbixFormat::Mask);
    assert!(strike.glyph(GlyphId(6)).is_none());
    assert!(strike.glyph(GlyphId(GLYPH_COUNT)).is_none());
    assert_eq!(sbix.glyph(GlyphId(1), 40).unwrap().format, SbixFormat::Jpeg);
    assert_eq!(sbix.glyph(GlyphId(1), 20).unwrap().ppem, 32);
    assert_eq!(sbix.glyph(GlyphId(5), 64).unwrap().ppem, 32);
    let table = sbix_table();
    for len in 0..table.len() {
        if let Some(sbix) = rikka_text_font::Sbix::new(&table[..len], GLYPH_COUNT) {
            for glyph in 0..=GLYPH_COUNT {
                let _ = sbix.glyph(GlyphId(glyph), 48);
            }
        }
    }
}

fn svg_table() -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(0).u32(10).u32(0);
    w.u16(2);
    w.u16(1).u16(3).u32(26).u32(6);
    w.u16(7).u16(7).u32(32).u32(4);
    w.raw(b"<svg/>").raw(&[0x1F, 0x8B, 8, 0]);
    w.finish()
}

#[test]
fn svg_documents_found_by_glyph_range() {
    let data = crate::support::sfnt(GLYPH_COUNT, &[(b"SVG ", svg_table())]);
    let font = Font::new(&data).unwrap();
    let plain = font.svg_document(GlyphId(2)).unwrap();
    assert_eq!(plain.data, b"<svg/>");
    assert_eq!(
        (plain.first_glyph, plain.last_glyph),
        (GlyphId(1), GlyphId(3))
    );
    assert!(!plain.is_compressed());
    assert!(font.svg_document(GlyphId(7)).unwrap().is_compressed());
    assert!(font.svg_document(GlyphId(0)).is_none());
    assert!(font.svg_document(GlyphId(5)).is_none());
    assert!(font.svg_document(GlyphId(8)).is_none());
    assert_eq!(font.svg().unwrap().len(), 2);
    let table = svg_table();
    for len in 0..table.len() {
        if let Some(svg) = rikka_text_font::Svg::new(&table[..len]) {
            for glyph in 0..10 {
                let _ = svg.document(GlyphId(glyph));
            }
        }
    }
}
