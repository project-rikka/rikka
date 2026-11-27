use std::path::PathBuf;

use crate::change::Change;
use crate::edit::Edit;
use crate::edited::Edited;
use crate::eol::Returns;
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;
use crate::files::Files;
use crate::history::History;
use crate::placement;
use crate::writer::{self, Precondition};

#[derive(Debug)]
pub struct EditRequest<'a> {
    pub path: &'a str,
    pub edits: Vec<Edit>,
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
        if request.edits.is_empty() {
            return Err(Error::NoEdits);
        }

        let path = self.resolve_inside(request.path)?;
        let relative_path = self.relative_path(&path);

        let lock = self.path_lock(&path);
        let _guard = lock.lock();

        let (bytes, text) = self.read_text(&path, &relative_path)?;
        let (before, returns) = Returns::strip(text.lines());
        let placements = placement::place(&relative_path, &request.edits, &before)?;
        let edited = Edited::apply(
            &relative_path,
            before,
            &returns,
            placements,
            request.edits.len(),
        );
        if edited.is_unchanged() {
            return Err(Error::Unchanged {
                path: relative_path.clone(),
                report: "The edits leave the file as it is.".to_owned(),
            });
        }
        edited.check_echoes()?;
        let result = text.with_exact_content(&edited.content(text.ends_with_newline()));
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
            excerpt: edited.excerpt_written(),
            change: edited.change(),
            path,
        })
    }
}
