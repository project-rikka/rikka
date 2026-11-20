const LATIN_SENTENCE: &str = "The quick brown fox jumps over the lazy dog, and office affluence keeps every typesetter busy. ";
const JAPANESE_SENTENCE: &str =
    "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。";
const MIXED_SENTENCE: &str =
    "The word مرحبا بالعالم means hello world in Arabic, ويستخدم كثيرا in examples. ";
const LABEL: &str = "Settings and privacy";
pub const PARAGRAPH_CHARACTERS: usize = 2000;

#[derive(Clone, Debug)]
pub struct ParagraphCase {
    pub name: &'static str,
    pub text: String,
}

pub fn paragraph_cases() -> Vec<ParagraphCase> {
    vec![
        ParagraphCase {
            name: "latin",
            text: repeated(LATIN_SENTENCE),
        },
        ParagraphCase {
            name: "japanese",
            text: repeated(JAPANESE_SENTENCE),
        },
        ParagraphCase {
            name: "arabic-english",
            text: repeated(MIXED_SENTENCE),
        },
        ParagraphCase {
            name: "label",
            text: LABEL.to_owned(),
        },
    ]
}

pub fn repeated(sentence: &str) -> String {
    let mut text = String::new();
    while text.chars().count() < PARAGRAPH_CHARACTERS {
        text.push_str(sentence);
    }
    text
}
