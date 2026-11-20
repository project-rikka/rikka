use std::path::PathBuf;

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];
const UNITS_PER_EM: u16 = 1000;

pub(crate) fn all_font_files() -> impl Iterator<Item = &'static PathBuf> {
    rikka_test_fonts::font_files().iter().filter(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                FONT_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
            })
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Path {
    pub(crate) segments: Vec<Segment>,
}

impl Path {
    pub(crate) fn contour_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| matches!(segment, Segment::Move(..)))
            .count()
    }

    pub(crate) fn bounds(&self) -> rikka_text_font::Bounds {
        let mut bounds = rikka_text_font::Bounds {
            x_min: f32::INFINITY,
            y_min: f32::INFINITY,
            x_max: f32::NEG_INFINITY,
            y_max: f32::NEG_INFINITY,
        };
        let mut include = |x: f32, y: f32| {
            bounds.x_min = bounds.x_min.min(x);
            bounds.y_min = bounds.y_min.min(y);
            bounds.x_max = bounds.x_max.max(x);
            bounds.y_max = bounds.y_max.max(y);
        };
        for segment in &self.segments {
            match *segment {
                Segment::Move(x, y) | Segment::Line(x, y) => include(x, y),
                Segment::Quad(x1, y1, x, y) => {
                    include(x1, y1);
                    include(x, y);
                }
                Segment::Cubic(x1, y1, x2, y2, x, y) => {
                    include(x1, y1);
                    include(x2, y2);
                    include(x, y);
                }
                Segment::Close => {}
            }
        }
        if bounds.x_min <= bounds.x_max && bounds.y_min <= bounds.y_max {
            bounds
        } else {
            rikka_text_font::Bounds::default()
        }
    }
}

impl rikka_text_font::OutlineSink for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.segments.push(Segment::Quad(x1, y1, x, y));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.segments.push(Segment::Cubic(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.segments.push(Segment::Close);
    }
}

pub(crate) fn sfnt(glyph_count: u16, tables: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut all = tables.to_vec();
    if !all.iter().any(|(tag, _)| *tag == b"head") {
        all.push((b"head", rikka_test_fonts::head(UNITS_PER_EM)));
    }
    if !all.iter().any(|(tag, _)| *tag == b"maxp") {
        all.push((b"maxp", rikka_test_fonts::maxp(glyph_count)));
    }
    rikka_test_fonts::sfnt(&all)
}

pub(crate) struct Mutator(u64);

impl Mutator {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub(crate) fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    pub(crate) fn corrupt(&mut self, data: &mut [u8], range: std::ops::Range<usize>, flips: usize) {
        for _ in 0..flips {
            let at = range.start + self.below(range.len());
            data[at] = self.next() as u8;
        }
    }
}

pub(crate) fn table_range(
    data: &[u8],
    face_offset: usize,
    tag: &[u8; 4],
) -> Option<std::ops::Range<usize>> {
    let count = u16::from_be_bytes([data[face_offset + 4], data[face_offset + 5]]) as usize;
    (0..count).find_map(|index| {
        let record = face_offset + 12 + index * 16;
        (&data[record..record + 4] == tag).then(|| {
            let read = |at: usize| {
                u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
            };
            read(record + 8)..read(record + 8) + read(record + 12)
        })
    })
}
