use std::fmt::Write;

use discord_core::{Block, Inline, Mention, Rich};
use discord_markdown::{parse, parse_line};

fn rendered(rich: &Rich) -> String {
    let mut out = String::new();
    blocks(&rich.0, &mut out);
    out
}

fn blocks(blocks: &[Block], out: &mut String) {
    for (at, block) in blocks.iter().enumerate() {
        if at > 0 {
            out.push(' ');
        }
        match block {
            Block::Paragraph(content) => wrapped("p", content, out),
            Block::Heading { level, content } => wrapped(&format!("h{level}"), content, out),
            Block::Subtext(content) => wrapped("sub", content, out),
            Block::Quote(inner) => {
                out.push_str("quote{");
                self::blocks(inner, out);
                out.push('}');
            }
            Block::Code { language, text } => {
                let language = language.as_deref().unwrap_or_default();
                write!(out, "code({language}){text:?}").unwrap();
            }
            Block::List { start, items } => {
                let start = start.map(|start| start.to_string()).unwrap_or_default();
                write!(out, "list({start}){{").unwrap();
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        out.push(';');
                    }
                    self::blocks(item, out);
                }
                out.push('}');
            }
        }
    }
}

fn wrapped(name: &str, content: &[Inline], out: &mut String) {
    out.push_str(name);
    out.push('[');
    inlines(content, out);
    out.push(']');
}

fn styled(name: &str, content: &[Inline], out: &mut String) {
    out.push_str(name);
    out.push('(');
    inlines(content, out);
    out.push(')');
}

fn inlines(content: &[Inline], out: &mut String) {
    for inline in content {
        match inline {
            Inline::Text(text) => out.push_str(text),
            Inline::Bold(inner) => styled("b", inner, out),
            Inline::Italic(inner) => styled("i", inner, out),
            Inline::Underline(inner) => styled("u", inner, out),
            Inline::Strike(inner) => styled("s", inner, out),
            Inline::Spoiler(inner) => styled("spoiler", inner, out),
            Inline::Code(code) => write!(out, "`{code}`").unwrap(),
            Inline::Link { url, label } => {
                write!(out, "link({url}|").unwrap();
                inlines(label, out);
                out.push(')');
            }
            Inline::Mention(Mention::User(user)) => write!(out, "@user:{}", user.get()).unwrap(),
            Inline::Mention(Mention::Role(role)) => write!(out, "@role:{}", role.get()).unwrap(),
            Inline::Mention(Mention::Channel(channel)) => {
                write!(out, "#{}", channel.get()).unwrap();
            }
            Inline::Mention(Mention::Everyone) => out.push_str("@everyone"),
            Inline::Mention(Mention::Here) => out.push_str("@here"),
            Inline::Emoji(emoji) => match emoji.id {
                Some(id) => {
                    let animated = if emoji.is_animated() { "a" } else { "" };
                    write!(out, ":{}:{}{animated}", emoji.name, id.get()).unwrap();
                }
                None => write!(out, "[{}]", emoji.name).unwrap(),
            },
            Inline::Time { unix, style } => write!(out, "time({unix},{style:?})").unwrap(),
            Inline::Break => out.push('/'),
        }
    }
}

#[test]
fn messages_parse_like_discord() {
    let cases: &[(&str, &str)] = &[
        ("**bold** and *it*", "p[b(bold) and i(it)]"),
        ("_under_score_", "p[_underi(score)]"),
        (
            "__under__ ~~gone~~ ||secret||",
            "p[u(under) s(gone) spoiler(secret)]",
        ),
        ("***both***", "p[i(b(both))]"),
        ("`co*de*`", "p[`co*de*`]"),
        ("\\*not italic\\*", "p[*not italic*]"),
        ("line one\nline two", "p[line one/line two]"),
        ("# Title\ntext", "h1[Title] p[text]"),
        ("### Three", "h3[Three]"),
        ("#no space", "p[#no space]"),
        ("-# small print", "sub[small print]"),
        ("> quoted\nafter", "quote{p[quoted]} p[after]"),
        ("- one\n- two", "list(){p[one];p[two]}"),
        ("1. first\n2. second", "list(1){p[first];p[second]}"),
        ("```rust\nfn x() {}\n```", "code(rust)\"fn x() {}\""),
        (
            "see https://rikka.sh/x.",
            "p[see link(https://rikka.sh/x|https://rikka.sh/x).]",
        ),
        (
            "<https://rikka.sh>",
            "p[link(https://rikka.sh|https://rikka.sh)]",
        ),
        (
            "[label](https://rikka.sh)",
            "p[link(https://rikka.sh|label)]",
        ),
        (
            "[label](javascript:alert(1))",
            "p[[label](javascript:alert(1))]",
        ),
        (
            "<@123> <@&456> <#789> @everyone @here",
            "p[@user:123 @role:456 #789 @everyone @here]",
        ),
        ("<:blob:42> <a:dance:43>", "p[:blob:42 :dance:43a]"),
        (
            "<t:1700000000:R> <t:1700000000>",
            "p[time(1700000000,Relative) time(1700000000,ShortDateTime)]",
        ),
        ("</ping:1>", "p[/ping]"),
    ];
    for (source, expected) in cases {
        assert_eq!(rendered(&parse(source)), *expected, "{source:?}");
    }
}

#[test]
fn single_lines_keep_block_markup_as_text() {
    let cases: &[(&str, &str)] = &[
        ("# not a heading", "p[# not a heading]"),
        ("> not a quote", "p[> not a quote]"),
        ("**still bold**", "p[b(still bold)]"),
        ("", ""),
    ];
    for (source, expected) in cases {
        assert_eq!(rendered(&parse_line(source)), *expected, "{source:?}");
    }
}
