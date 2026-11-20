use std::sync::Arc;
use std::time::Duration;

use cut_project::Project;
use cut_render::{Request, Sequence};
use rikka_image_edit::Imaging;
use rikka_types::CancelToken;

use super::{Report, Reporter, Summary};
use crate::error::Result;
use crate::output::OutputFile;
use crate::settings::Still;

pub(super) fn still(
    project: &Arc<Project>,
    frame: i64,
    still: &Still,
    output: OutputFile,
    imaging: &Imaging,
    report: &Reporter,
    cancel: &CancelToken,
) -> Result<Option<Summary>> {
    let (width, height) = still.resolution.size(project.format());
    report(Report::Opened { backend: None });
    let mut sequence = Sequence::new(imaging.clone(), cancel.clone());
    let request = Request::new(Arc::clone(project), frame).sized(width, height);
    let Some(composite) = sequence.render(&request)? else {
        return Ok(None);
    };
    report(Report::Previewed(composite.picture.shown()?));
    report(Report::Finishing);
    composite.picture.save(output.draft())?;
    let path = output.commit()?;
    report(Report::Advanced { done: 1 });
    Ok(Some(Summary {
        path,
        duration: Duration::ZERO,
        frames: 1,
        backend: None,
        sound: false,
        undecodable: 0,
        offline: composite.offline.len(),
    }))
}
