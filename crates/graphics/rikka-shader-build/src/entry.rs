#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Stage {
    Vertex,
    Fragment,
    Compute,
}

impl Stage {
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Vertex => "vert",
            Self::Fragment => "frag",
            Self::Compute => "comp",
        }
    }

    fn from_slang(name: &str) -> Option<Self> {
        match name {
            "vertex" => Some(Self::Vertex),
            "fragment" => Some(Self::Fragment),
            "compute" => Some(Self::Compute),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryPoint {
    pub(crate) name: String,
    pub(crate) stage: Stage,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntryFault {
    UnknownStage(String),
    Unnamed(Stage),
}

const ATTRIBUTE: &str = "[shader(\"";

pub(crate) fn entry_points(source: &str) -> Result<Vec<EntryPoint>, EntryFault> {
    let code = without_comments(source);
    let mut found = Vec::new();
    let mut rest = code.as_str();
    while let Some(start) = rest.find(ATTRIBUTE) {
        let after = &rest[start + ATTRIBUTE.len()..];
        let Some(quote) = after.find('"') else {
            return Err(EntryFault::UnknownStage(after.to_owned()));
        };
        let stage_name = &after[..quote];
        let stage = Stage::from_slang(stage_name)
            .ok_or_else(|| EntryFault::UnknownStage(stage_name.to_owned()))?;
        let declaration = after[quote..]
            .find(']')
            .map_or("", |close| &after[quote + close + 1..]);
        let name = function_name(declaration).ok_or(EntryFault::Unnamed(stage))?;
        found.push(EntryPoint { name, stage });
        rest = declaration;
    }
    Ok(found)
}

fn function_name(declaration: &str) -> Option<String> {
    let mut text = declaration.trim_start();
    while text.starts_with('[') {
        let mut depth = 0usize;
        let close = text.char_indices().find_map(|(at, character)| {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(at);
                    }
                }
                _ => {}
            }
            None
        })?;
        text = text[close + 1..].trim_start();
    }
    let head = &text[..text.find('(')?];
    head.split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .rfind(|token| !token.is_empty())
        .map(str::to_owned)
}

fn without_comments(source: &str) -> String {
    let mut code = String::with_capacity(source.len());
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(line) = rest.strip_prefix("//") {
            rest = line.find('\n').map_or("", |end| &line[end..]);
        } else if let Some(block) = rest.strip_prefix("/*") {
            rest = block.find("*/").map_or("", |end| &block[end + 2..]);
            code.push(' ');
        } else {
            let character = rest.chars().next().unwrap_or_default();
            code.push(character);
            rest = &rest[character.len_utf8()..];
        }
    }
    code
}
