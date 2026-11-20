use rikka_text_font::{Font, FontFile, GlyphId, Location, OutlineScratch, OutlineSink, Tag};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Command {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Default)]
struct Path(Vec<Command>);

impl OutlineSink for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(Command::Quad(x1, y1, x, y));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Command::Cubic(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(Command::Close);
    }
}

impl Path {
    fn points(&self) -> Vec<(f32, f32)> {
        self.0
            .iter()
            .flat_map(|command| match *command {
                Command::Move(x, y) | Command::Line(x, y) => vec![(x, y)],
                Command::Quad(a, b, x, y) => vec![(a, b), (x, y)],
                Command::Cubic(a, b, c, d, x, y) => vec![(a, b), (c, d), (x, y)],
                Command::Close => vec![],
            })
            .collect()
    }

    fn bounds(&self) -> Option<[f32; 4]> {
        let points = self.points();
        let first = points.first()?;
        Some(
            points
                .iter()
                .fold([first.0, first.1, first.0, first.1], |b, &(x, y)| {
                    [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
                }),
        )
    }

    fn contours(&self) -> usize {
        self.0
            .iter()
            .filter(|command| matches!(command, Command::Move(..)))
            .count()
    }

    fn tight_bounds(&self) -> Option<[f32; 4]> {
        let mut points = Vec::new();
        let mut current = (0.0, 0.0);
        for command in &self.0 {
            match *command {
                Command::Move(x, y) | Command::Line(x, y) | Command::Quad(_, _, x, y) => {
                    current = (x, y)
                }
                Command::Cubic(a, b, c, d, x, y) => {
                    for step in 1..16 {
                        let t = step as f32 / 16.0;
                        let u = 1.0 - t;
                        points.push((
                            u * u * u * current.0
                                + 3.0 * u * u * t * a
                                + 3.0 * u * t * t * c
                                + t * t * t * x,
                            u * u * u * current.1
                                + 3.0 * u * u * t * b
                                + 3.0 * u * t * t * d
                                + t * t * t * y,
                        ));
                    }
                    current = (x, y);
                }
                Command::Close => continue,
            }
            points.push(current);
        }
        let first = *points.first()?;
        Some(
            points
                .iter()
                .fold([first.0, first.1, first.0, first.1], |b, &(x, y)| {
                    [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
                }),
        )
    }

    fn area(&self) -> f32 {
        let mut area = 0.0;
        let mut current = (0.0, 0.0);
        let mut start = (0.0, 0.0);
        let mut edge = |from: (f32, f32), to: (f32, f32)| area += from.0 * to.1 - to.0 * from.1;
        for command in &self.0 {
            match *command {
                Command::Move(x, y) => {
                    start = (x, y);
                    current = start;
                }
                Command::Line(x, y) => {
                    edge(current, (x, y));
                    current = (x, y);
                }
                Command::Quad(_, _, x, y) => {
                    edge(current, (x, y));
                    current = (x, y);
                }
                Command::Cubic(a, b, c, d, x, y) => {
                    let from = current;
                    for step in 1..=8 {
                        let t = step as f32 / 8.0;
                        let u = 1.0 - t;
                        let point = (
                            u * u * u * from.0
                                + 3.0 * u * u * t * a
                                + 3.0 * u * t * t * c
                                + t * t * t * x,
                            u * u * u * from.1
                                + 3.0 * u * u * t * b
                                + 3.0 * u * t * t * d
                                + t * t * t * y,
                        );
                        edge(current, point);
                        current = point;
                    }
                }
                Command::Close => {
                    edge(current, start);
                    current = start;
                }
            }
        }
        area.abs() / 2.0
    }
}

fn outline(font: &Font<'_>, glyph: u16, location: &Location) -> Option<Path> {
    let mut path = Path::default();
    font.outline(
        GlyphId(glyph),
        location,
        &mut OutlineScratch::default(),
        &mut path,
    )
    .ok()?;
    Some(path)
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn i16_at(data: &[u8], offset: usize) -> i16 {
    u16_at(data, offset) as i16
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn glyf_bounds(font: &Font<'_>, glyph: u16) -> Option<[f32; 4]> {
    let head = font.table(Tag::new(b"head")).unwrap();
    let loca = font.table(Tag::new(b"loca")).unwrap();
    let glyf = font.table(Tag::new(b"glyf")).unwrap();
    let (start, end) = if i16_at(head, 50) == 0 {
        let at = usize::from(glyph) * 2;
        (
            usize::from(u16_at(loca, at)) * 2,
            usize::from(u16_at(loca, at + 2)) * 2,
        )
    } else {
        let at = usize::from(glyph) * 4;
        (u32_at(loca, at) as usize, u32_at(loca, at + 4) as usize)
    };
    (end > start).then(|| [2, 4, 6, 8].map(|offset| f32::from(i16_at(glyf, start + offset))))
}

#[test]
fn cff_outlines_match_truetype_build() {
    let otf = rikka_test_fonts::font_bytes("JetBrainsMono-Regular.otf");
    let ttf = rikka_test_fonts::font_bytes("JetBrainsMono-Regular.ttf");
    let otf = Font::new(&otf).unwrap();
    let ttf = Font::new(&ttf).unwrap();
    assert_eq!(otf.glyph_count(), ttf.glyph_count());
    let mut compared = 0;
    for glyph in 0..otf.glyph_count() {
        let path = outline(&otf, glyph, &Location::default()).unwrap();
        match (path.tight_bounds(), glyf_bounds(&ttf, glyph)) {
            (Some(cubic), Some(quadratic)) => {
                for (a, b) in cubic.iter().zip(quadratic) {
                    assert!(
                        (a - b).abs() <= 24.0,
                        "glyph {glyph}: {cubic:?} vs {quadratic:?}"
                    );
                }
                compared += 1;
            }
            (None, None) => {}
            (cubic, quadratic) => panic!("glyph {glyph}: {cubic:?} vs {quadratic:?}"),
        }
    }
    assert!(compared > 1000);
}

#[test]
fn cff2_outlines_follow_weight_axis() {
    let data = rikka_test_fonts::font_bytes("NotoSansCJK-VF.otf.ttc");
    let file = FontFile::new(&data).unwrap();
    let font = file.face(0).unwrap();
    assert!(font.cff().is_none());
    let head = font.table(Tag::new(b"head")).unwrap();
    let font_box = [36, 38, 40, 42].map(|offset| f32::from(i16_at(head, offset)));
    let axis = font.fvar().unwrap().axis(0).unwrap();
    assert_eq!(
        (axis.tag, axis.min, axis.max),
        (Tag::new(b"wght"), 100.0, 900.0)
    );
    let thin = font.location([(Tag::new(b"wght"), 100.0)]);
    let medium = font.location([(Tag::new(b"wght"), 500.0)]);
    let black = font.location([(Tag::new(b"wght"), 900.0)]);
    assert_ne!(thin, black);
    let mut heavier = 0;
    let mut compared = 0;
    for glyph in (1..font.glyph_count()).step_by(97) {
        let regular = outline(&font, glyph, &medium).unwrap();
        let light = outline(&font, glyph, &thin).unwrap();
        let bold = outline(&font, glyph, &black).unwrap();
        for path in [&regular, &light, &bold] {
            if let Some(bounds) = path.bounds() {
                assert!(
                    bounds[0] >= font_box[0] && bounds[1] >= font_box[1],
                    "glyph {glyph}"
                );
                assert!(
                    bounds[2] <= font_box[2] && bounds[3] <= font_box[3],
                    "glyph {glyph}"
                );
            }
        }
        if regular.0.is_empty() {
            continue;
        }
        compared += 1;
        assert_eq!(light.contours(), bold.contours());
        if bold.area() > regular.area() && regular.area() > light.area() {
            heavier += 1;
        }
    }
    assert!(compared > 500);
    assert!(heavier * 10 >= compared * 9, "{heavier} of {compared}");
}

fn corrupt_and_draw(name: &str, table: &[u8; 4], rounds: u32) {
    let original = rikka_test_fonts::font_bytes(name);
    let font = Font::new(&original).unwrap();
    let table = font.table(Tag::new(table)).unwrap();
    let start = table.as_ptr() as usize - original.as_ptr() as usize;
    let length = table.len();
    let glyphs = font.glyph_count();
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut data = original.clone();
    for _ in 0..rounds {
        data.copy_from_slice(&original);
        for _ in 0..16 {
            let at = start + (next() as usize % length.min(4096));
            data[at] = next() as u8;
        }
        for _ in 0..8 {
            let at = start + (next() as usize % length);
            data[at] = next() as u8;
        }
        let font = Font::new(&data).unwrap();
        for _ in 0..32 {
            let glyph = (next() % u32::from(glyphs)) as u16;
            let _ = outline(&font, glyph, &Location::default());
            if let Some(cff) = font.cff() {
                let _ = cff.advance_width(GlyphId(glyph));
                let _ = cff.glyph_name(GlyphId(glyph));
            }
        }
    }
}

#[test]
fn corrupted_cff_data_never_panics() {
    corrupt_and_draw("JetBrainsMono-Regular.otf", b"CFF ", 300);
    corrupt_and_draw("Font Awesome 7 Free-Regular-400.otf", b"CFF ", 300);
}

#[test]
fn corrupted_cff2_data_never_panics() {
    corrupt_and_draw("NotoSansMonoCJK-VF.otf.ttc", b"CFF2", 40);
}

struct FontBuilder {
    tables: Vec<([u8; 4], Vec<u8>)>,
}

impl FontBuilder {
    fn build(mut self, glyph_count: u16) -> Vec<u8> {
        let mut head = vec![0u8; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        let mut maxp = 0x0000_5000u32.to_be_bytes().to_vec();
        maxp.extend_from_slice(&glyph_count.to_be_bytes());
        self.tables.push((*b"head", head));
        self.tables.push((*b"maxp", maxp));
        self.tables.sort_by_key(|(tag, _)| *tag);
        let mut out = b"OTTO".to_vec();
        out.extend_from_slice(&(self.tables.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0; 6]);
        let mut offset = 12 + 16 * self.tables.len();
        let mut body = Vec::new();
        for (tag, data) in &self.tables {
            out.extend_from_slice(tag);
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            body.extend_from_slice(data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            offset = 12 + 16 * self.tables.len() + body.len();
        }
        out.extend_from_slice(&body);
        out
    }
}

fn number(value: i16) -> Vec<u8> {
    let [high, low] = value.to_be_bytes();
    vec![28, high, low]
}

fn offset(value: usize) -> Vec<u8> {
    let mut bytes = vec![29];
    bytes.extend_from_slice(&(value as i32).to_be_bytes());
    bytes
}

fn charstring(parts: &[&[i16]], ops: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for (operands, operator) in parts.iter().zip(ops) {
        for &value in *operands {
            out.extend(number(value));
        }
        out.extend_from_slice(operator);
    }
    out
}

fn index(items: &[Vec<u8>]) -> Vec<u8> {
    if items.is_empty() {
        return vec![0, 0];
    }
    let mut out = (items.len() as u16).to_be_bytes().to_vec();
    out.push(4);
    let mut position = 1u32;
    out.extend_from_slice(&position.to_be_bytes());
    for item in items {
        position += item.len() as u32;
        out.extend_from_slice(&position.to_be_bytes());
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

const RMOVETO: &[u8] = &[21];
const RLINETO: &[u8] = &[5];
const CALLSUBR: &[u8] = &[10];
const RETURN: &[u8] = &[11];
const ENDCHAR: &[u8] = &[14];

fn cff_table(
    top: impl Fn(&[usize]) -> Vec<u8>,
    sections: impl Fn(&[usize]) -> Vec<Vec<u8>>,
) -> Vec<u8> {
    let header = vec![1u8, 0, 4, 4];
    let names = index(&[b"Test".to_vec()]);
    let draft = sections(&[0; 8]);
    let top_size = index(&[top(&[0; 8])]).len();
    let mut positions = Vec::new();
    let mut at = header.len() + names.len() + top_size + 2 + 2;
    for section in &draft {
        positions.push(at);
        at += section.len();
    }
    let mut out = header;
    out.extend(names);
    out.extend(index(&[top(&positions)]));
    out.extend(index(&[]));
    out.extend(index(&[]));
    for section in sections(&positions) {
        out.extend(section);
    }
    out
}

fn seac_font() -> Vec<u8> {
    let subrs = index(&[charstring(&[&[-400, 0], &[]], &[RLINETO, RETURN])]);
    let mut private = Vec::new();
    private.extend(number(500));
    private.push(20);
    private.extend(number(600));
    private.push(21);
    let private_size = private.len() + 6;
    private.extend(offset(private_size));
    private.push(19);
    let mut private_section = private.clone();
    private_section.extend(subrs);
    let charstrings = index(&[
        ENDCHAR.to_vec(),
        charstring(
            &[&[50, 100, 0], &[200, 700, 200, -700], &[-107], &[]],
            &[RMOVETO, RLINETO, CALLSUBR, ENDCHAR],
        ),
        charstring(
            &[&[200, 600], &[100, 100, -50, 0], &[]],
            &[RMOVETO, RLINETO, ENDCHAR],
        ),
        charstring(&[&[10, 50, 120, 65, 194]], &[ENDCHAR]),
    ]);
    let charset = vec![0, 0, 34, 0, 125, 0, 171];
    let cff = cff_table(
        |at| {
            let mut top = Vec::new();
            top.extend(offset(at[0]));
            top.push(15);
            top.extend(offset(at[1]));
            top.push(17);
            top.extend(offset(private.len()));
            top.extend(offset(at[2]));
            top.push(18);
            top
        },
        |_| {
            vec![
                charset.clone(),
                charstrings.clone(),
                private_section.clone(),
            ]
        },
    );
    FontBuilder {
        tables: vec![(*b"CFF ", cff)],
    }
    .build(4)
}

fn real(nibbles: &[u8]) -> Vec<u8> {
    let mut out = vec![30];
    let mut padded = nibbles.to_vec();
    padded.push(0xF);
    if padded.len() % 2 == 1 {
        padded.push(0xF);
    }
    out.extend(padded.chunks(2).map(|pair| (pair[0] << 4) | pair[1]));
    out
}

fn font_matrix(scale: &[u8]) -> Vec<u8> {
    let zero = number(0);
    let mut out = Vec::new();
    for part in [scale, &zero, &zero, scale, &zero, &zero] {
        out.extend_from_slice(part);
    }
    out.extend_from_slice(&[12, 7]);
    out
}

fn font_dict(scale: &[u8], private: &[u8], private_at: usize) -> Vec<u8> {
    let mut dict = font_matrix(scale);
    dict.extend(offset(private.len()));
    dict.extend(offset(private_at));
    dict.push(18);
    dict
}

fn cid_font() -> Vec<u8> {
    let mut first_private = number(0);
    first_private.push(21);
    let mut second_private = number(1000);
    second_private.push(21);
    let subrs_at = second_private.len() + 6;
    second_private.extend(offset(subrs_at));
    second_private.push(19);
    let mut second_section = second_private.clone();
    second_section.extend(index(&[charstring(&[&[100, 0], &[]], &[RLINETO, RETURN])]));
    let square = charstring(
        &[&[100, 100], &[100, 0, 0, 100], &[]],
        &[RMOVETO, RLINETO, ENDCHAR],
    );
    let called = charstring(
        &[&[7, 100, 100], &[-107], &[0, 100], &[]],
        &[RMOVETO, CALLSUBR, RLINETO, ENDCHAR],
    );
    let charstrings = index(&[ENDCHAR.to_vec(), square, called]);
    let fd_select = vec![3, 0, 2, 0, 0, 0, 0, 2, 1, 0, 3];
    let charset = vec![0, 0, 1, 0, 2];
    let thousandth = real(&[0, 0xA, 0, 0, 1]);
    let two_thousandths = real(&[0, 0xA, 0, 0, 2]);
    let cff = cff_table(
        |at| {
            let mut top = Vec::new();
            top.extend(number(391));
            top.extend(number(392));
            top.extend(number(0));
            top.extend_from_slice(&[12, 30]);
            top.extend(offset(at[0]));
            top.push(15);
            top.extend(offset(at[1]));
            top.push(17);
            top.extend(offset(at[2]));
            top.extend_from_slice(&[12, 37]);
            top.extend(offset(at[3]));
            top.extend_from_slice(&[12, 36]);
            top
        },
        |at| {
            let fd_array = index(&[
                font_dict(&thousandth, &first_private, at[4]),
                font_dict(&two_thousandths, &second_private, at[5]),
            ]);
            vec![
                charset.clone(),
                charstrings.clone(),
                fd_select.clone(),
                fd_array,
                first_private.clone(),
                second_section.clone(),
            ]
        },
    );
    FontBuilder {
        tables: vec![(*b"CFF ", cff)],
    }
    .build(3)
}

#[test]
fn seac_composes_base_and_accent() {
    let data = seac_font();
    let font = Font::new(&data).unwrap();
    let cff = font.cff().unwrap();
    assert_eq!(cff.glyph_name(GlyphId(1)), Some("A"));
    assert_eq!(cff.glyph_name(GlyphId(2)), Some("acute"));
    assert_eq!(cff.glyph_name(GlyphId(3)), Some("Aacute"));
    let base = outline(&font, 1, &Location::default()).unwrap();
    assert_eq!(
        base.0,
        [
            Command::Move(100.0, 0.0),
            Command::Line(300.0, 700.0),
            Command::Line(500.0, 0.0),
            Command::Line(100.0, 0.0),
            Command::Close,
        ]
    );
    let composed = outline(&font, 3, &Location::default()).unwrap();
    assert_eq!(
        composed.0,
        [
            Command::Move(250.0, 720.0),
            Command::Line(350.0, 820.0),
            Command::Line(300.0, 820.0),
            Command::Close,
            Command::Move(100.0, 0.0),
            Command::Line(300.0, 700.0),
            Command::Line(500.0, 0.0),
            Command::Line(100.0, 0.0),
            Command::Close,
        ]
    );
    assert_eq!(cff.advance_width(GlyphId(0)), Some(500.0));
    assert_eq!(cff.advance_width(GlyphId(1)), Some(650.0));
    assert_eq!(cff.advance_width(GlyphId(2)), Some(500.0));
    assert_eq!(cff.advance_width(GlyphId(3)), Some(610.0));
}

#[test]
fn cid_keyed_fonts_select_font_dict() {
    let data = cid_font();
    let font = Font::new(&data).unwrap();
    let cff = font.cff().unwrap();
    assert!(cff.is_cid_keyed());
    assert_eq!(cff.glyph_name(GlyphId(1)), None);
    let square = outline(&font, 1, &Location::default()).unwrap();
    assert_eq!(
        square.0,
        [
            Command::Move(100.0, 100.0),
            Command::Line(200.0, 100.0),
            Command::Line(200.0, 200.0),
            Command::Close,
        ]
    );
    let doubled = outline(&font, 2, &Location::default()).unwrap();
    assert_eq!(
        doubled.0,
        [
            Command::Move(200.0, 200.0),
            Command::Line(400.0, 200.0),
            Command::Line(400.0, 400.0),
            Command::Close,
        ]
    );
    assert_eq!(cff.advance_width(GlyphId(2)), Some(2014.0));
}
