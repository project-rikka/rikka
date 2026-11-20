use rikka_text_layout::Wrap;
use rikka_text_unicode::line_breaks;

use crate::support::{family, layout_text, row_texts, wrapped};

const THAI: &str = "ภาษาไทยเป็นภาษาที่มีระดับเสียงของคำแน่นอนหรือวรรณยุกต์เช่นเดียวกับภาษาจีนและออกเสียงแยกคำต่อคำ";
const LAO: &str = "ເຈົ້າເວົ້າພາສາອັງກິດໄດ້ບໍ່ກະລຸນາເວົ້າຊ້າໆ";
const KHMER: &str = "សូមចំណាយពេលបន្តិចដើម្បីអធិស្ឋានអរព្រះគុណដល់ព្រះអង្គ";
const BURMESE: &str = "ထိုသူတို့သည်အချင်းချင်းမေတ္တာထား၍ဆက်ဆံကျင့်သုံး";

fn row_ends(text: &str, width: f32) -> Vec<usize> {
    let layout = layout_text(text, &wrapped(width, Wrap::Word));
    for glyph in layout.all_glyphs() {
        assert_ne!(glyph.id, 0, "{text} has a missing glyph");
        assert!(
            family(glyph.face).starts_with("Noto Sans") || family(glyph.face) == "Inter",
            "{text} falls back to {}",
            family(glyph.face)
        );
    }
    assert_eq!(row_texts(&layout).concat(), text);
    layout
        .rows()
        .iter()
        .map(|row| row.text_range().end)
        .collect()
}

fn assert_wraps_between_words(text: &str, widths: &[f32]) {
    let opportunities: Vec<usize> = line_breaks(text)
        .map(|line_break| line_break.offset)
        .collect();
    assert!(opportunities.len() > 4, "{text} has no word boundaries");
    for &width in widths {
        let ends = row_ends(text, width);
        assert!(ends.len() > 1, "{text} fits in one row at {width} px");
        for end in ends {
            assert!(
                opportunities.contains(&end),
                "a row at {width} px ends inside a word: {:?} | {:?}",
                &text[..end],
                &text[end..]
            );
        }
    }
}

#[test]
fn thai_wraps_between_dictionary_words() {
    let paragraph = [THAI, THAI, THAI].join(" ");
    assert_wraps_between_words(&paragraph, &[90.0, 160.0, 240.0, 410.0]);
}

#[test]
fn lao_khmer_burmese_wrap_between_words() {
    for text in [LAO, KHMER, BURMESE] {
        assert_wraps_between_words(text, &[80.0, 150.0]);
    }
}
