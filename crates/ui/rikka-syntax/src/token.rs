use rikka_theme::Token;

const NAMES: &[(&str, Option<Token>)] = &[
    ("attribute", Some(Token::Attribute)),
    ("boolean", Some(Token::Constant)),
    ("character", Some(Token::String)),
    ("character.special", Some(Token::Escape)),
    ("charset", Some(Token::Keyword)),
    ("comment", Some(Token::Comment)),
    ("conceal", None),
    ("conditional", Some(Token::Keyword)),
    ("constant", Some(Token::Constant)),
    ("constructor", Some(Token::Type)),
    ("delimiter", Some(Token::Punctuation)),
    ("embedded", None),
    ("error", Some(Token::Error)),
    ("escape", Some(Token::Escape)),
    ("exception", Some(Token::Keyword)),
    ("field", Some(Token::Property)),
    ("float", Some(Token::Number)),
    ("function", Some(Token::Function)),
    ("import", Some(Token::Keyword)),
    ("include", Some(Token::Keyword)),
    ("keyframes", Some(Token::Keyword)),
    ("keyword", Some(Token::Keyword)),
    ("label", Some(Token::Constant)),
    ("media", Some(Token::Keyword)),
    ("method", Some(Token::Function)),
    ("module", Some(Token::Namespace)),
    ("namespace", Some(Token::Namespace)),
    ("none", None),
    ("number", Some(Token::Number)),
    ("operator", Some(Token::Operator)),
    ("parameter", Some(Token::Parameter)),
    ("preproc", Some(Token::Keyword)),
    ("property", Some(Token::Property)),
    ("punctuation", Some(Token::Punctuation)),
    ("punctuation.special", Some(Token::Escape)),
    ("repeat", Some(Token::Keyword)),
    ("spell", None),
    ("storageclass", Some(Token::Keyword)),
    ("string", Some(Token::String)),
    ("string.escape", Some(Token::Escape)),
    ("string.special", Some(Token::String)),
    ("string.special.symbol", Some(Token::Constant)),
    ("supports", Some(Token::Keyword)),
    ("symbol", Some(Token::Constant)),
    ("tag", Some(Token::Tag)),
    ("tag.error", Some(Token::Error)),
    ("text", None),
    ("text.literal", Some(Token::String)),
    ("text.reference", Some(Token::Property)),
    ("text.title", Some(Token::Keyword)),
    ("text.uri", Some(Token::Function)),
    ("type", Some(Token::Type)),
    ("variable", Some(Token::Variable)),
    ("variable.builtin", Some(Token::Constant)),
    ("variable.member", Some(Token::Property)),
    ("variable.parameter", Some(Token::Parameter)),
];

pub(crate) fn token_of(capture: &str) -> Option<Token> {
    let mut name = capture;
    loop {
        if let Ok(at) = NAMES.binary_search_by_key(&name, |(key, _)| *key) {
            return NAMES[at].1;
        }
        name = &name[..name.rfind('.')?];
    }
}
