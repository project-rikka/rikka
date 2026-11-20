use rikka_text_font::{FontFile, Tag};

use crate::support::{Request, shape_with};
use rikka_test_fonts::{loaded_font, without_table};

struct Corruptor(u64);

impl Corruptor {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn table_ranges(data: &[u8], tags: &[&[u8; 4]]) -> Vec<std::ops::Range<usize>> {
    let file = FontFile::new(data).unwrap();
    let font = file.face(0).unwrap();
    tags.iter()
        .filter_map(|tag| font.table(Tag::new(tag)))
        .map(|table| {
            let start = table.as_ptr() as usize - data.as_ptr() as usize;
            start..start + table.len()
        })
        .collect()
}

fn corrupt_and_shape(
    original: &[u8],
    tables: &[&[u8; 4]],
    text: &str,
    seed: u64,
    request: &Request<'_>,
) {
    let ranges = table_ranges(original, tables);
    let mut corruptor = Corruptor(seed | 1);
    for _ in 0..60 {
        let mut data = original.to_vec();
        for _ in 0..24 {
            let range = &ranges[corruptor.next() as usize % ranges.len()];
            let at = range.start + corruptor.next() as usize % range.len().max(1);
            data[at] = corruptor.next() as u8;
        }
        if FontFile::new(&data).and_then(|file| file.face(0)).is_err() {
            continue;
        }
        shape_with(data.leak(), text, request);
    }
}

#[test]
fn corrupted_layout_never_panics_in_shapers() {
    for (seed, (file_name, text)) in [
        ("NotoSans.ttf", "office affluent AVATAR é ñ 1/2"),
        ("NotoNaskhArabic.ttf", "بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ لا"),
        ("NotoNastaliqUrdu.ttf", "پاکستان زندہ باد"),
        ("NotoSansDevanagari.ttf", "र्क्षि क्\u{200D}ष ि\u{94D} द्ध्र्य"),
        ("NotoSansBengali.ttf", "ন্ত্র ব্য্র কো র\u{200D}্য"),
        ("NotoSansKhmer.ttf", "ស្ត្រី ក្រ កើ ្ក"),
        ("NotoSansMyanmar.ttf", "သင်္ဘော ကြွ \u{103C}"),
        ("NotoSansJavanese.ttf", "ꦲꦏ꧀ꦱꦫ ꦏꦿꦺ ꦁꦏ"),
        ("NotoSerifTibetan.ttf", "བསྒྲུབས་ ཧཱུྃ \u{F71}"),
    ]
    .into_iter()
    .enumerate()
    {
        corrupt_and_shape(
            loaded_font(file_name),
            &[b"GSUB", b"GPOS", b"GDEF", b"cmap", b"hmtx"],
            text,
            7 + 4 * seed as u64,
            &Request::default(),
        );
    }
}

#[test]
fn corrupted_gvar_never_panics_on_advances() {
    let data = without_table(loaded_font("InterVariable.ttf"), b"HVAR")
        .expect("Inter has no table directory");
    corrupt_and_shape(
        &data,
        &[b"gvar", b"glyf", b"loca"],
        "Wavy office AVATAR",
        41,
        &Request {
            variations: &[(b"wght", 720.0), (b"opsz", 30.0)],
            ..Request::default()
        },
    );
}

#[test]
fn unusual_sequences_never_panic() {
    let data = loaded_font("NotoSans.ttf");
    let texts = [
        "\u{301}\u{302}\u{303}",
        "\u{200D}\u{200C}\u{200D}",
        "\u{FE0F}\u{FE0F}\u{E0100}",
        "\u{2044}\u{2044}1\u{2044}",
        "\u{34F}\u{301}\u{34F}",
        "a\u{E0020}\u{E007F}",
        "\u{1F3FB}\u{1F1E6}",
    ];
    for text in texts {
        for flags in [
            rikka_text_shape::BufferFlags::empty(),
            rikka_text_shape::BufferFlags::BEGINNING_OF_TEXT
                | rikka_text_shape::BufferFlags::END_OF_TEXT,
            rikka_text_shape::BufferFlags::PRODUCE_UNSAFE_TO_CONCAT
                | rikka_text_shape::BufferFlags::PRESERVE_DEFAULT_IGNORABLES,
        ] {
            shape_with(
                data,
                text,
                &Request {
                    flags,
                    ..Request::default()
                },
            );
        }
    }
}
