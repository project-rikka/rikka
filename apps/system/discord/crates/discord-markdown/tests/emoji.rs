use discord_core::{Block, Inline};
use discord_markdown::parse_line;

fn pieces(text: &str) -> Vec<String> {
    let rich = parse_line(text);
    let Some(Block::Paragraph(inlines)) = rich.0.first() else {
        return Vec::new();
    };
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Emoji(emoji) => format!("[{}]", emoji.name),
            Inline::Text(text) => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn unicode_emoji_split_like_discord() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("presentation", "hi 😀", &["hi ", "[😀]"]),
        ("text default", "© 2026", &["© 2026"]),
        ("text default with selector", "©\u{FE0F}", &["[©\u{FE0F}]"]),
        (
            "emoji default with text selector",
            "☔\u{FE0E}",
            &["☔\u{FE0E}"],
        ),
        ("plain digits", "1 # *", &["1 # *"]),
        ("keycap", "1\u{20E3}", &["[1\u{20E3}]"]),
        (
            "keycap with selector",
            "#\u{FE0F}\u{20E3}",
            &["[#\u{FE0F}\u{20E3}]"],
        ),
        ("digit selector without keycap", "1\u{FE0F}", &["1\u{FE0F}"]),
        ("flag", "🇯🇵", &["[🇯🇵]"]),
        ("lone regional indicator", "🇦 b", &["[🇦]", " b"]),
        ("modifier on base", "👋🏽", &["[👋🏽]"]),
        ("modifier on text-default base", "☝🏻", &["[☝🏻]"]),
        ("modifier on non-base", "❤🏻", &["❤", "[🏻]"]),
        (
            "zwj family",
            "👨\u{200D}👩\u{200D}👧",
            &["[👨\u{200D}👩\u{200D}👧]"],
        ),
        (
            "tag flag",
            "🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
            &["[🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}]"],
        ),
    ];
    for (name, text, expected) in cases {
        assert_eq!(pieces(text), *expected, "{name}");
    }
}
