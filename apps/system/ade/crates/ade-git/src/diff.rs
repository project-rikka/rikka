use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Place(String),
    Added { content: String, at: u32 },
    Removed { content: String, at: u32 },
    Context { content: String, was: u32, at: u32 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    path: PathBuf,
    from: Option<PathBuf>,
    lines: Vec<Line>,
    binary: bool,
    added: usize,
    removed: usize,
}

impl Diff {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn from(&self) -> Option<&Path> {
        self.from.as_deref()
    }

    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.binary
    }

    #[must_use]
    pub fn added(&self) -> usize {
        self.added
    }

    #[must_use]
    pub fn removed(&self) -> usize {
        self.removed
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn take(&mut self, line: &str, at: &mut Place) {
        if line.starts_with('\\') {
            return;
        }
        let (mark, content) = line.split_at(line.len().min(1));
        let content = content.to_owned();
        match mark {
            "+" => {
                self.lines.push(Line::Added {
                    content,
                    at: at.new,
                });
                self.added += 1;
                at.new += 1;
            }
            "-" => {
                self.lines.push(Line::Removed {
                    content,
                    at: at.old,
                });
                self.removed += 1;
                at.old += 1;
            }
            " " | "" => {
                self.lines.push(Line::Context {
                    content,
                    was: at.old,
                    at: at.new,
                });
                at.old += 1;
                at.new += 1;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Patch {
    files: Vec<Diff>,
}

impl Patch {
    #[must_use]
    pub fn files(&self) -> &[Diff] {
        &self.files
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub fn added(&self) -> usize {
        self.files.iter().map(Diff::added).sum()
    }

    #[must_use]
    pub fn removed(&self) -> usize {
        self.files.iter().map(Diff::removed).sum()
    }

    #[must_use]
    pub fn parse(patch: &str) -> Patch {
        let mut files: Vec<Diff> = Vec::new();
        let mut place = None;

        for line in patch.lines() {
            if let Some(names) = opening(line) {
                files.push(names);
                place = None;
                continue;
            }
            let Some(file) = files.last_mut() else {
                continue;
            };
            if is_binary(line) {
                file.binary = true;
                continue;
            }
            if let Some(header) = chunk(line) {
                file.lines.push(Line::Place(header.tail));
                place = Some(header.at);
                continue;
            }
            if let Some(at) = place.as_mut() {
                file.take(line, at);
            }
        }

        Patch { files }
    }

    pub(crate) fn rename(&mut self, path: &Path) {
        if let [file] = self.files.as_mut_slice() {
            file.path = path.to_owned();
            file.from = None;
        }
    }
}

fn opening(line: &str) -> Option<Diff> {
    let names = line.strip_prefix("diff --git ")?;
    let (before, after) = split_names(names)?;
    Some(Diff {
        from: (before != after).then(|| PathBuf::from(&before)),
        path: PathBuf::from(after),
        ..Diff::default()
    })
}

fn split_names(names: &str) -> Option<(String, String)> {
    let at = names.rfind(" b/")?;
    let before = names[..at].strip_prefix("a/")?;
    let after = &names[at + " b/".len()..];
    Some((before.to_owned(), after.to_owned()))
}

struct Place {
    old: u32,
    new: u32,
}

struct Header {
    at: Place,
    tail: String,
}

fn chunk(line: &str) -> Option<Header> {
    let rest = line.strip_prefix("@@ ")?;
    let (spans, tail) = rest.split_once(" @@")?;
    let (old, new) = spans.split_once(' ')?;
    Some(Header {
        at: Place {
            old: start(old.strip_prefix('-')?),
            new: start(new.strip_prefix('+')?),
        },
        tail: tail.trim().to_owned(),
    })
}

fn start(span: &str) -> u32 {
    span.split(',')
        .next()
        .and_then(|at| at.parse().ok())
        .unwrap_or(1)
}

fn is_binary(line: &str) -> bool {
    line.starts_with("GIT binary patch")
        || (line.starts_with("Binary files ") && line.ends_with(" differ"))
}
