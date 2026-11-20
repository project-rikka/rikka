use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static DRAFTS: AtomicU64 = AtomicU64::new(0);
const DRAFT_STEM: usize = 48;

#[derive(Debug)]
pub(crate) struct Landing {
    folder: PathBuf,
    name: String,
    draft: PathBuf,
    directory: bool,
    settled: bool,
}

impl Landing {
    pub(crate) fn file(folder: &Path, name: &str) -> Landing {
        let extension = Path::new(name)
            .extension()
            .map_or_else(String::new, |extension| {
                format!(".{}", extension.to_string_lossy())
            });
        Landing::drafted(folder, name, extension, false)
    }

    pub(crate) fn directory(folder: &Path, name: &str) -> std::io::Result<Landing> {
        let landing = Landing::drafted(folder, name, String::new(), true);
        std::fs::create_dir(&landing.draft)?;
        Ok(landing)
    }

    fn drafted(folder: &Path, name: &str, extension: String, directory: bool) -> Landing {
        let number = DRAFTS.fetch_add(1, Ordering::Relaxed);
        let stem: String = rikka_format::name_stem(name)
            .chars()
            .take(DRAFT_STEM)
            .collect();
        let draft = folder.join(format!(
            ".{stem}.converting-{}-{number}{extension}",
            std::process::id()
        ));
        Landing {
            folder: folder.to_path_buf(),
            name: name.to_owned(),
            draft,
            directory,
            settled: false,
        }
    }

    pub(crate) fn draft(&self) -> &Path {
        &self.draft
    }

    pub(crate) fn settle(mut self) -> std::io::Result<PathBuf> {
        loop {
            let name = rikka_format::unique_name(&self.name, |existing_name| {
                self.folder.join(existing_name).symlink_metadata().is_ok()
            });
            let target = self.folder.join(name);
            match rikka_fs::rename_new(&self.draft, &target) {
                Ok(()) => {
                    self.settled = true;
                    return Ok(target);
                }
                Err(existing_name) if existing_name.kind() == ErrorKind::AlreadyExists => {}
                Err(refused) => return Err(refused),
            }
        }
    }
}

impl Drop for Landing {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        let removed = match self.directory {
            true => std::fs::remove_dir_all(&self.draft),
            false => std::fs::remove_file(&self.draft),
        };
        if let Err(error) = removed
            && error.kind() != ErrorKind::NotFound
        {
            tracing::warn!(%error, draft = %self.draft.display(), "failed to remove draft");
        }
    }
}
