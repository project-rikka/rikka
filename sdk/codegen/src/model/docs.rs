use crate::diagnostic::{Diagnostic, Result, Span};
use crate::model::{Callback, Class, Constant, Enum, Flags, Function, Id, Struct};
use crate::syntax::ast::{DocLine, Name};

#[derive(Debug, Default)]
pub(crate) struct Doc {
    pub(crate) body: Vec<Paragraph>,
    pub(crate) returns: Paragraph,
    pub(crate) set: Paragraph,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Paragraph(Vec<Inline>);

#[derive(Clone, Debug)]
pub(crate) enum Inline {
    Text(String),
    Reference(Target),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Target {
    Constant(Id<Constant>),
    Struct(Id<Struct>),
    Field(Id<Struct>, usize),
    Enum(Id<Enum>),
    Variant(Id<Enum>, usize),
    Flags(Id<Flags>),
    Flag(Id<Flags>, usize),
    Callback(Id<Callback>),
    Class(Id<Class>),
    Member(Id<Class>, usize),
    Function(Id<Function>),
}

impl Paragraph {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn render(&self, link: &dyn Fn(Target) -> String) -> String {
        self.0
            .iter()
            .map(|inline| match inline {
                Inline::Text(text) => text.clone(),
                Inline::Reference(target) => link(*target),
            })
            .collect()
    }

    fn push_text(&mut self, text: &str) {
        match self.0.last_mut() {
            Some(Inline::Text(last)) => last.push_str(text),
            _ => self.0.push(Inline::Text(text.to_owned())),
        }
    }
}

pub(super) struct Fragment {
    text: String,
    span: Span,
}

pub(super) struct Tagged {
    pub(super) span: Span,
    pub(super) fragments: Vec<Fragment>,
}

#[derive(Default)]
pub(super) struct Sections {
    pub(super) body: Vec<Vec<Fragment>>,
    pub(super) params: Vec<(Name, Vec<Fragment>)>,
    pub(super) returns: Option<Tagged>,
    pub(super) set: Option<Tagged>,
}

enum Open {
    Body,
    Param,
    Returns,
    Set,
    Closed,
}

pub(super) fn sections(lines: &[DocLine]) -> Result<Sections> {
    let mut sections = Sections::default();
    let mut open = Open::Closed;
    for line in lines {
        let text = line.text.as_str();
        if text.is_empty() {
            open = Open::Closed;
            continue;
        }
        let fragment = |skipped: usize| {
            let trimmed = text[skipped..].trim_start();
            let offset = text.len() - trimmed.len();
            Fragment {
                text: trimmed.to_owned(),
                span: line
                    .span
                    .shifted(offset, line.span.length.saturating_sub(offset)),
            }
        };
        if let Some(rest) = text.strip_prefix("@param ") {
            let rest = rest.trim_start();
            let name_length = rest
                .find(|character: char| character.is_whitespace())
                .unwrap_or(rest.len());
            let offset = text.len() - rest.len();
            let name = Name {
                text: rest[..name_length].to_owned(),
                span: line.span.shifted(offset, name_length),
            };
            if let Some((existing, _)) = sections
                .params
                .iter()
                .find(|(existing, _)| existing.text == name.text)
            {
                return Err(Diagnostic::new(
                    name.span,
                    format!("`@param {}` is repeated", existing.text),
                ));
            }
            sections
                .params
                .push((name, vec![fragment(offset + name_length)]));
            open = Open::Param;
        } else if let Some((tag, slot, state)) = [
            ("@returns ", &mut sections.returns, Open::Returns),
            ("@set ", &mut sections.set, Open::Set),
        ]
        .into_iter()
        .find_map(|(tag, slot, state)| text.starts_with(tag).then_some((tag, slot, state)))
        {
            if slot.is_some() {
                return Err(Diagnostic::new(
                    line.span,
                    format!("`{}` is repeated", tag.trim_end()),
                ));
            }
            *slot = Some(Tagged {
                span: line.span.shifted(0, tag.len() - 1),
                fragments: vec![fragment(tag.len())],
            });
            open = state;
        } else if text.starts_with('@') {
            return Err(Diagnostic::new(
                line.span.shifted(0, text.find(' ').unwrap_or(text.len())),
                "unknown doc tag; expected `@param`, `@returns` or `@set`",
            ));
        } else {
            let continued = fragment(0);
            match open {
                Open::Body => sections
                    .body
                    .last_mut()
                    .expect("an open body paragraph exists")
                    .push(continued),
                Open::Param => sections
                    .params
                    .last_mut()
                    .expect("an open parameter exists")
                    .1
                    .push(continued),
                Open::Returns => sections
                    .returns
                    .as_mut()
                    .expect("an open @returns exists")
                    .fragments
                    .push(continued),
                Open::Set => sections
                    .set
                    .as_mut()
                    .expect("an open @set exists")
                    .fragments
                    .push(continued),
                Open::Closed
                    if sections.params.is_empty()
                        && sections.returns.is_none()
                        && sections.set.is_none() =>
                {
                    sections.body.push(vec![continued]);
                    open = Open::Body;
                }
                Open::Closed => {
                    return Err(Diagnostic::new(
                        line.span,
                        "doc text after a tag; write the description before `@param`, `@returns` and `@set`",
                    ));
                }
            }
        }
    }
    Ok(sections)
}

pub(super) fn paragraph(
    fragments: &[Fragment],
    resolve: &dyn Fn(&str, Span) -> Result<Target>,
) -> Result<Paragraph> {
    let mut paragraph = Paragraph::default();
    for (index, fragment) in fragments.iter().enumerate() {
        if index > 0 {
            paragraph.push_text(" ");
        }
        let mut rest = fragment.text.as_str();
        let mut column = fragment.span.column;
        while let Some(start) = rest.find(['[', '\\']) {
            paragraph.push_text(&rest[..start]);
            let marker = &rest[start..];
            if let Some(escaped) = marker.strip_prefix("\\[") {
                paragraph.push_text("[");
                column += rest.chars().count() - escaped.chars().count();
                rest = escaped;
                continue;
            }
            if let Some(after) = marker.strip_prefix('\\') {
                paragraph.push_text("\\");
                column += rest.chars().count() - after.chars().count();
                rest = after;
                continue;
            }
            let reference_column = column + rest[..start].chars().count();
            let Some(end) = marker.find(']') else {
                return Err(Diagnostic::new(
                    Span::new(fragment.span.line, reference_column, 1),
                    "`[` opens a reference that is not closed on its line; write `\\[` for a bracket",
                ));
            };
            let name = &marker[1..end];
            let span = Span::new(
                fragment.span.line,
                reference_column,
                name.chars().count() + 2,
            );
            paragraph.0.push(Inline::Reference(resolve(name, span)?));
            let after = &marker[end + 1..];
            column += rest.chars().count() - after.chars().count();
            rest = after;
        }
        paragraph.push_text(rest);
    }
    Ok(paragraph)
}
