use std::io;
use std::path::{Path, PathBuf};

use rikka_fs::{Replace, Staged};

#[derive(Debug)]
pub(crate) struct OutputFile {
    staged: Staged,
    overwrite: bool,
}

impl OutputFile {
    pub(crate) fn new(target: &Path, overwrite: bool) -> io::Result<OutputFile> {
        Ok(OutputFile {
            staged: Replace::new(target).keep_extension().stage()?,
            overwrite,
        })
    }

    pub(crate) fn draft(&self) -> &Path {
        self.staged.path()
    }

    pub(crate) fn commit(self) -> io::Result<PathBuf> {
        let target = self.staged.target().to_path_buf();
        if self.overwrite {
            self.staged.commit()?;
            return Ok(target);
        }
        let numbered = numbered_beside(&target);
        self.staged
            .commit_unused(std::iter::once(target).chain(numbered))
    }
}

fn numbered_beside(target: &Path) -> impl Iterator<Item = PathBuf> + use<> {
    let folder = target.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = target
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    (2..).map(move |index| folder.join(rikka_format::numbered_name(&name, index)))
}
