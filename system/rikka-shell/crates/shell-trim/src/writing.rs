use std::ops::Range;
use std::path::Path;
use std::time::Duration;

use rikka_session::replay::{Keep, Saved};

use crate::error::{Error, Result};
use crate::replays;

pub(crate) fn keep(draft: &Path, span: Range<Duration>) -> Result<Saved> {
    Ok(replays::keep(&Keep {
        draft: draft.to_path_buf(),
        from: span.start.as_secs_f64(),
        to: span.end.as_secs_f64(),
    })?)
}

pub(crate) fn write(file: &Path, span: Range<Duration>) -> Result<()> {
    let cut = rikka_fs::Replace::new(file)
        .keep_extension()
        .stage()
        .map_err(Error::NotReplaced)?;
    let trimmed_file = rikka_video::trim(file, cut.path(), span)?;
    cut.commit().map_err(Error::NotReplaced)?;
    tracing::info!(
        "trimmed {} to {:?}",
        file.display(),
        trimmed_file.end - trimmed_file.start
    );
    Ok(())
}
