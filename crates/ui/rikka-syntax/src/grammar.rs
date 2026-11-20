use std::sync::OnceLock;

use rikka_theme::Token;
use tree_sitter as ts;

use crate::comment::Comment;
use crate::queries;
use crate::token::token_of;
use crate::words::QUOTES;

pub(crate) struct Rules {
    pub(crate) name: &'static str,
    pub(crate) language: ts::Language,
    pub(crate) highlights: ts::Query,
    pub(crate) tokens: Box<[Option<Token>]>,
    pub(crate) injections: Option<InjectionQuery>,
    pub(crate) keywords: Box<[Box<str>]>,
}

pub(crate) struct InjectionQuery {
    pub(crate) query: ts::Query,
    pub(crate) content: u32,
    pub(crate) language: Option<u32>,
}

pub(crate) struct Grammar {
    pub(crate) name: &'static str,
    pub(crate) comment: Option<Comment>,
    pub(crate) joiners: &'static str,
    pub(crate) quotes: &'static str,
    pub(crate) openers: &'static str,
    highlights: &'static [&'static str],
    injections: &'static [&'static str],
    rules: OnceLock<Option<Rules>>,
}

impl Grammar {
    const fn new(name: &'static str, highlights: &'static [&'static str]) -> Grammar {
        Grammar {
            name,
            comment: None,
            joiners: "",
            quotes: QUOTES,
            openers: "",
            highlights,
            injections: &[],
            rules: OnceLock::new(),
        }
    }

    const fn injections(mut self, injections: &'static [&'static str]) -> Grammar {
        self.injections = injections;
        self
    }

    const fn line_comment(mut self, starts: &'static str) -> Grammar {
        self.comment = Some(Comment::Line(starts));
        self
    }

    const fn block_comment(mut self, opens: &'static str, closes: &'static str) -> Grammar {
        self.comment = Some(Comment::Block(opens, closes));
        self
    }

    const fn joiners(mut self, joiners: &'static str) -> Grammar {
        self.joiners = joiners;
        self
    }

    const fn quotes(mut self, quotes: &'static str) -> Grammar {
        self.quotes = quotes;
        self
    }

    const fn openers(mut self, openers: &'static str) -> Grammar {
        self.openers = openers;
        self
    }

    pub(crate) fn rules(&'static self) -> Option<&'static Rules> {
        self.rules.get_or_init(|| self.compile()).as_ref()
    }

    fn compile(&self) -> Option<Rules> {
        let language = rikka_grammars::Grammar::all()
            .find(|grammar| grammar.name() == self.name)?
            .language()?
            .clone();
        let highlights = compiled(&language, self.highlights, self.name, "highlights")?;
        let tokens = highlights
            .capture_names()
            .iter()
            .map(|name| token_of(name))
            .collect();
        let injections = compiled(&language, self.injections, self.name, "injections")
            .and_then(|query| InjectionQuery::new(query, self.name));
        let keywords = keywords_of(&language);
        Some(Rules {
            name: self.name,
            language,
            highlights,
            tokens,
            injections,
            keywords,
        })
    }
}

fn keywords_of(language: &ts::Language) -> Box<[Box<str>]> {
    let mut kinds: Vec<&str> = (0..language.node_kind_count())
        .filter_map(|id| u16::try_from(id).ok())
        .filter(|&id| !language.node_kind_is_named(id) && language.node_kind_is_visible(id))
        .filter_map(|id| language.node_kind_for_id(id))
        .filter(|kind| is_keyword(kind))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds.into_iter().map(Box::from).collect()
}

fn is_keyword(kind: &str) -> bool {
    let mut letters = kind.chars();
    kind.len() >= 2
        && letters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
        && letters.all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
}

impl InjectionQuery {
    fn new(query: ts::Query, language: &str) -> Option<InjectionQuery> {
        let slot = |capture: &str| {
            query
                .capture_names()
                .iter()
                .position(|name| *name == capture)
                .map(|at| at as u32)
        };
        let content = slot("injection.content");
        let named = slot("injection.language");
        let Some(content) = content else {
            tracing::warn!("{language} injections query has no @injection.content capture");
            return None;
        };
        Some(InjectionQuery {
            query,
            content,
            language: named,
        })
    }
}

fn compiled(
    language: &ts::Language,
    sources: &[&str],
    name: &str,
    what: &str,
) -> Option<ts::Query> {
    if sources.is_empty() {
        return None;
    }
    match ts::Query::new(language, &sources.join("\n")) {
        Ok(query) => Some(query),
        Err(error) => {
            tracing::warn!("failed to compile {name} {what}: {error}");
            None
        }
    }
}

pub(crate) static GRAMMARS: [Grammar; 26] = [
    Grammar::new("bash", &[queries::BASH_HIGHLIGHTS])
        .line_comment("#")
        .quotes("\"'`"),
    Grammar::new("c", &[queries::C_HIGHLIGHTS]).line_comment("//"),
    Grammar::new("cpp", &[queries::C_HIGHLIGHTS, queries::CPP_HIGHLIGHTS]).line_comment("//"),
    Grammar::new("csharp", &[queries::C_SHARP_HIGHLIGHTS]).line_comment("//"),
    Grammar::new("css", &[queries::CSS_HIGHLIGHTS])
        .block_comment("/*", "*/")
        .joiners("-"),
    Grammar::new("diff", &[queries::DIFF_HIGHLIGHTS]),
    Grammar::new("go", &[queries::GO_HIGHLIGHTS])
        .line_comment("//")
        .quotes("\"'`"),
    Grammar::new("html", &[queries::HTML_HIGHLIGHTS])
        .injections(&[queries::HTML_INJECTIONS])
        .block_comment("<!--", "-->")
        .joiners("-"),
    Grammar::new("java", &[queries::JAVA_HIGHLIGHTS]).line_comment("//"),
    Grammar::new(
        "javascript",
        &[
            queries::JAVASCRIPT_HIGHLIGHTS,
            queries::JAVASCRIPT_JSX_HIGHLIGHTS,
        ],
    )
    .injections(&[queries::JAVASCRIPT_INJECTIONS])
    .line_comment("//")
    .joiners("$")
    .quotes("\"'`"),
    Grammar::new("json", &[queries::JSON_HIGHLIGHTS]),
    Grammar::new("lua", &[queries::LUA_HIGHLIGHTS])
        .injections(&[queries::LUA_INJECTIONS])
        .line_comment("--"),
    Grammar::new("markdown", &[queries::MARKDOWN_HIGHLIGHTS])
        .injections(&[queries::MARKDOWN_INJECTIONS])
        .block_comment("<!--", "-->")
        .quotes("\"`"),
    Grammar::new("markdown_inline", &[queries::MARKDOWN_INLINE_HIGHLIGHTS])
        .injections(&[queries::MARKDOWN_INLINE_INJECTIONS]),
    Grammar::new("nix", &[queries::NIX_HIGHLIGHTS])
        .injections(&[queries::NIX_INJECTIONS])
        .line_comment("#")
        .joiners("-'"),
    Grammar::new("php", &[queries::PHP_HIGHLIGHTS])
        .injections(&[queries::PHP_INJECTIONS])
        .line_comment("//")
        .joiners("$"),
    Grammar::new("python", &[queries::PYTHON_HIGHLIGHTS])
        .line_comment("#")
        .openers(":"),
    Grammar::new("ruby", &[queries::RUBY_HIGHLIGHTS]).line_comment("#"),
    Grammar::new("rust", &[queries::RUST_HIGHLIGHTS])
        .injections(&[queries::RUST_INJECTIONS])
        .line_comment("//")
        .quotes("\""),
    Grammar::new("slang", &[queries::C_HIGHLIGHTS, include_str!("slang.scm")]).line_comment("//"),
    Grammar::new("sql", &[queries::SQL_HIGHLIGHTS]).line_comment("--"),
    Grammar::new("starlark", &[queries::STARLARK_HIGHLIGHTS])
        .injections(&[queries::STARLARK_INJECTIONS])
        .line_comment("#")
        .openers(":"),
    Grammar::new("toml", &[queries::TOML_HIGHLIGHTS])
        .line_comment("#")
        .joiners("-"),
    Grammar::new(
        "tsx",
        &[
            queries::JAVASCRIPT_HIGHLIGHTS,
            queries::JAVASCRIPT_JSX_HIGHLIGHTS,
            queries::TYPESCRIPT_HIGHLIGHTS,
        ],
    )
    .injections(&[queries::JAVASCRIPT_INJECTIONS])
    .line_comment("//")
    .joiners("$")
    .quotes("\"'`"),
    Grammar::new(
        "typescript",
        &[
            queries::JAVASCRIPT_HIGHLIGHTS,
            queries::TYPESCRIPT_HIGHLIGHTS,
        ],
    )
    .injections(&[queries::JAVASCRIPT_INJECTIONS])
    .line_comment("//")
    .joiners("$")
    .quotes("\"'`"),
    Grammar::new("yaml", &[queries::YAML_HIGHLIGHTS])
        .line_comment("#")
        .openers(":")
        .joiners("-"),
];
