use std::path::PathBuf;

use crate::change::Change;
use crate::edited::Edited;
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;
use crate::files::Files;
use crate::history::History;
use crate::op::{self, Op, ResolvedOp};
use crate::writer::{self, Precondition};

#[derive(Debug)]
pub struct EditRequest<'a> {
    pub path: &'a str,
    pub ops: Vec<Op>,
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
        if request.ops.is_empty() {
            return Err(Error::NoOps);
        }

        let path = self.resolve_inside(request.path)?;
        let relative_path = self.relative_path(&path);

        let lock = self.path_lock(&path);
        let _guard = lock.lock();

        let (bytes, text) = self.read_text(&path, &relative_path)?;
        let resolved = request
            .ops
            .iter()
            .map(|op| op.resolved(&text, &relative_path))
            .collect::<Result<Vec<ResolvedOp<'_>>>>()?;
        let order = op::sort_disjoint(&resolved)?;

        let edited = Edited::apply(&relative_path, text.lines().collect(), &resolved, &order);
        edited.check_echoes()?;
        let result = text.with_lines(&edited.after);
        edited.check_syntax(&path, text.content(), result.content())?;

        if let Some(history) = history {
            history.record(&path, Some(&bytes))?;
        }
        writer::write(&path, &result.render(), &Precondition::Exactly(&bytes))?;

        let written: Vec<&str> = result.lines().collect();
        let (before, after) = (edited.before.len(), written.len());
        Ok(EditOutcome {
            summary: format!(
                "ok {relative_path} ({after} lines, {:+})",
                after as isize - before as isize
            ),
            excerpt: edited.excerpt_after(&written),
            change: Change::diff(relative_path.clone(), &edited.before, &written),
            path,
        })
    }
}
