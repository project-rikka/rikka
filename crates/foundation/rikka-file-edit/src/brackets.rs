use std::path::Path;

use rikka_grammars::Depths;

const BRACKET_CHECKED: &[&str] = &[
    "glsl", "vert", "frag", "comp", "geom", "tesc", "tese", "mesh", "task", "rgen", "rint",
    "rahit", "rchit", "rmiss", "rcall", "wgsl", "hlsl", "metal", "cu",
];

enum LexState {
    Code,
    LineComment,
    BlockComment,
    Quoted(char),
}

pub(crate) fn bracket_problems(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            BRACKET_CHECKED
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

pub(crate) fn bracket_depths(text: &str) -> Depths {
    let mut depths = Depths::default();
    let mut state = LexState::Code;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        state = match (state, ch) {
            (LexState::Code, '/') if chars.next_if_eq(&'/').is_some() => LexState::LineComment,
            (LexState::Code, '/') if chars.next_if_eq(&'*').is_some() => LexState::BlockComment,
            (LexState::Code, quote @ ('"' | '\'')) => LexState::Quoted(quote),
            (LexState::Code, ch) => {
                depths.count(ch);
                LexState::Code
            }
            (LexState::LineComment, '\n') => LexState::Code,
            (LexState::BlockComment, '*') if chars.next_if_eq(&'/').is_some() => LexState::Code,
            (LexState::Quoted(quote), '\\') => {
                chars.next();
                LexState::Quoted(quote)
            }
            (LexState::Quoted(quote), ch) if ch == quote || ch == '\n' => LexState::Code,
            (state, _) => state,
        };
    }
    depths
}
