use std::ops::Range;
use std::path::PathBuf;

use crate::change::Change;
use crate::cut::Cut;
use crate::edited::Edited;
use crate::eol::Returns;
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;
use crate::files::Files;
use crate::history::History;
use crate::matching;
use crate::refusal::Place;
use crate::replacement::Replacement;
use crate::seam;
use crate::writer::{self, Precondition};

#[derive(Debug)]
pub struct EditRequest<'a> {
    pub path: &'a str,
    pub replacements: Vec<Replacement>,
}

#[derive(Debug)]
pub struct EditOutcome {
    pub path: PathBuf,
    pub summary: String,
    pub excerpt: Excerpt,
    pub change: Change,
}

impl Files {
    pub fn edit(&self, request: &EditRequest<'_>) -> Result<EditOutcome> {
        self.edit_recorded(request, None)
    }

    pub(crate) fn edit_recorded(
        &self,
        request: &EditRequest<'_>,
        history: Option<&History>,
    ) -> Result<EditOutcome> {
        if request.replacements.is_empty() {
            return Err(Error::NoEdits);
        }

        let path = self.resolve_inside(request.path)?;
        let relative_path = self.relative_path(&path);

        let lock = self.path_lock(&path);
        let _guard = lock.lock();

        let (bytes, text) = self.read_text(&path, &relative_path)?;
        let content = replaced(&relative_path, text.content(), &request.replacements)?;
        let result = text.with_exact_content(&content);
        let edited = Edited::between(
            &relative_path,
            text.lines().collect(),
            result.lines().collect(),
        );
        edited.check_syntax(&path, text.content(), result.content())?;

        if let Some(history) = history {
            history.record(&path, Some(&bytes))?;
        }
        writer::write(&path, &result.render(), &Precondition::Exactly(&bytes))?;

        let (before, after) = (edited.before.len(), edited.after.len());
        Ok(EditOutcome {
            summary: format!(
                "ok {relative_path} ({after} lines, {:+})",
                after as isize - before as isize
            ),
            excerpt: edited.excerpt_after(),
            change: edited.change(),
            path,
        })
    }
}

fn replaced(path: &str, original: &str, replacements: &[Replacement]) -> Result<String> {
    let (mut content, mut returns) = Returns::strip(original);
    let mut echoes: Vec<(Range<usize>, String)> = Vec::new();

    for (index, replacement) in replacements.iter().enumerate() {
        let place = Place::new(index, replacements.len());
        let old = replacement.old.replace("\r\n", "\n");
        let new = replacement.new.replace("\r\n", "\n");
        if old.is_empty() {
            return Err(Error::EmptyOld {
                path: path.to_owned(),
                report: place.empty(),
            });
        }
        if old == new {
            return Err(Error::Unchanged {
                path: path.to_owned(),
                report: place.unchanged(),
            });
        }

        let found = match replacement.all {
            true => matching::occurrences(&content, &old),
            false => matching::overlapping(&content, &old),
        };
        match (found.as_slice(), replacement.all) {
            ([], _) => {
                return Err(Error::NoMatch {
                    path: path.to_owned(),
                    report: place.no_match(&content, &old),
                });
            }
            ([at], _) => {
                if let Some(echo) = seam::echo(&content, *at..*at + old.len(), &new) {
                    echoes.push((echo.neighbour.clone(), place.echoed(&content, &echo)));
                }
            }
            (_, false) => {
                return Err(Error::Ambiguous {
                    path: path.to_owned(),
                    report: place.ambiguous(&content, &found),
                });
            }
            (_, true) => {}
        }

        for &at in found.iter().rev() {
            let cut = Cut {
                at: at..at + old.len(),
                added: new.len(),
            };
            returns.replace(&cut, &old, &new);
            echoes = echoes
                .into_iter()
                .filter_map(|(line, report)| Some((cut.moved_line(&line)?, report)))
                .collect();
        }
        content = spliced(&content, &found, old.len(), &new);
    }

    match echoes.is_empty() {
        true => Ok(returns.restore(&content)),
        false => Err(Error::EchoedLine {
            path: path.to_owned(),
            report: echoes
                .into_iter()
                .map(|(_, report)| report)
                .collect::<Vec<String>>()
                .join("\n\n"),
        }),
    }
}

fn spliced(content: &str, found: &[usize], width: usize, new: &str) -> String {
    let mut spliced = String::with_capacity(content.len() + found.len() * new.len());
    let mut at = 0;
    for &start in found {
        spliced.push_str(&content[at..start]);
        spliced.push_str(new);
        at = start + width;
    }
    spliced.push_str(&content[at..]);
    spliced
}
