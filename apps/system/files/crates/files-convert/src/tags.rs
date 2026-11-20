use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::TagExt;

pub(crate) fn copy_tags(source: &Path, written: &Path) {
    let read = match lofty::read_from_path(source) {
        Ok(read) => read,
        Err(unread) => {
            tracing::debug!(error = %unread, source = %source.display(), "no tags to copy");
            return;
        }
    };
    let Some(tag) = read.primary_tag().or_else(|| read.first_tag()) else {
        return;
    };
    let kind = Probe::open(written)
        .ok()
        .and_then(|probe| probe.guess_file_type().ok())
        .and_then(|probe| probe.file_type());
    let Some(kind) = kind else {
        tracing::warn!(path = %written.display(), "format does not support tags");
        return;
    };
    let mut tag = tag.clone();
    tag.re_map(kind.primary_tag_type());
    if let Err(refused) = tag.save_to_path(written, WriteOptions::default()) {
        tracing::warn!(error = %refused, source = %source.display(), "failed to copy tags");
    }
}
