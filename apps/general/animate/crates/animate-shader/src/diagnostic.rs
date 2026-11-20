use rikka_shader_graph::{GraphError, NodeId, Scope};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    Graph,
    Scope(Scope),
    Node {
        scope: Scope,
        node: NodeId,
        port: Option<String>,
    },
    Line {
        line: u32,
        column: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub at: Location,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn graph(message: impl Into<String>) -> Self {
        Self {
            at: Location::Graph,
            message: message.into(),
        }
    }

    pub fn node(&self) -> Option<(&Scope, NodeId)> {
        match &self.at {
            Location::Node { scope, node, .. } => Some((scope, *node)),
            _ => None,
        }
    }

    pub fn line(&self) -> Option<u32> {
        match self.at {
            Location::Line { line, .. } => Some(line),
            _ => None,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.at {
            Location::Line { line, column } => {
                write!(formatter, "line {line}, column {column}: {}", self.message)
            }
            _ => formatter.write_str(&self.message),
        }
    }
}

pub(crate) fn from_graph(errors: &[GraphError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| Diagnostic {
            at: match (error.at.node, error.at.scope.clone()) {
                (Some(node), scope) => Location::Node {
                    scope,
                    node,
                    port: error.at.port.clone(),
                },
                (None, Scope::Main) => Location::Graph,
                (None, scope) => Location::Scope(scope),
            },
            message: match &error.at.port {
                Some(port) => format!("{port}: {}", error.problem),
                None => error.problem.to_string(),
            },
        })
        .collect()
}

struct Block<'a> {
    title: &'a str,
    file: Option<&'a str>,
    line: u32,
    column: u32,
    caret: Option<&'a str>,
}

fn blocks(text: &str) -> Vec<Block<'_>> {
    let mut blocks: Vec<Block<'_>> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = ["error[", "warning[", "fatal error["]
            .iter()
            .find_map(|prefix| trimmed.strip_prefix(prefix))
        {
            let title = rest.split_once("]: ").map_or(rest, |(_, title)| title);
            blocks.push(Block {
                title,
                file: None,
                line: 0,
                column: 0,
                caret: None,
            });
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        if let Some(place) = trimmed.strip_prefix("--> ") {
            let mut parts = place.rsplitn(3, ':');
            let column = parts.next().and_then(|part| part.parse().ok());
            let row = parts.next().and_then(|part| part.parse().ok());
            if let (Some(column), Some(row), Some(file)) = (column, row, parts.next()) {
                block.file = Some(file);
                block.line = row;
                block.column = column;
            }
            continue;
        }
        if block.caret.is_none()
            && let Some((_, after)) = trimmed.split_once('|')
        {
            let after = after.trim_start();
            if after.starts_with('^') {
                let message = after.trim_start_matches(['^', '~', '-']).trim();
                if !message.is_empty() {
                    block.caret = Some(message);
                }
            }
        }
    }
    blocks
}

pub(crate) fn from_slang(text: &str, module: &str, lines: usize) -> Vec<Diagnostic> {
    let blocks = blocks(text);
    let own = |block: &Block<'_>| {
        block.file.is_some_and(|file| {
            file.strip_suffix(".slang")
                .and_then(|stem| stem.rsplit_once('-'))
                .is_some_and(|(name, number)| {
                    name == module && number.chars().all(|ch| ch.is_ascii_digit())
                })
        }) && block.line >= 1
            && block.line as usize <= lines
    };
    let located: Vec<Diagnostic> = blocks
        .iter()
        .filter(|block| own(block))
        .map(|block| Diagnostic {
            at: Location::Line {
                line: block.line,
                column: block.column,
            },
            message: block.caret.unwrap_or(block.title).to_owned(),
        })
        .collect();
    if !located.is_empty() {
        let mut unique = Vec::with_capacity(located.len());
        for diagnostic in located {
            if !unique.contains(&diagnostic) {
                unique.push(diagnostic);
            }
        }
        return unique;
    }
    match blocks.first() {
        Some(block) => vec![Diagnostic::graph(block.caret.unwrap_or(block.title))],
        None => vec![Diagnostic::graph(text.trim())],
    }
}

pub(crate) fn from_build(
    error: &rikka_shader_compile::Error,
    module: &str,
    lines: usize,
) -> Vec<Diagnostic> {
    match error {
        rikka_shader_compile::Error::Graph(error) => from_graph(error.errors()),
        rikka_shader_compile::Error::Renderer(rikka_renderer_3d::Error::Slang(
            rikka_slang::Error::Compile { diagnostics },
        )) => from_slang(diagnostics, module, lines),
        rikka_shader_compile::Error::Renderer(other) => vec![Diagnostic::graph(other.to_string())],
    }
}
