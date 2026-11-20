use std::fs::Metadata;
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use rikka_proton_drive_sync_state::LocalItem;
use rustix::fs::{CWD, RenameFlags, renameat_with};
use rustix::io::Errno;

use crate::local::files::{TempFile, modified, move_to_available_name, sync_folder};

pub(crate) struct DownloadedFile {
    pub(crate) temporary: TempFile,
    pub(crate) folder: PathBuf,
    pub(crate) name: String,
    pub(crate) modified: i64,
}

pub(crate) enum Placement {
    Placed { path: PathBuf, metadata: Metadata },
    ChangedLocally,
}

enum ExchangeOutcome {
    Replaced,
    Refused,
    RestoreFailed,
}

impl DownloadedFile {
    pub(crate) fn place(self, replacing: Option<&LocalItem>) -> io::Result<Placement> {
        if self.modified > 0 {
            rikka_fs::set_modified(
                self.temporary.path(),
                UNIX_EPOCH + Duration::from_secs(self.modified.unsigned_abs()),
            )?;
        }
        let path = match replacing {
            None => self.folder.join(move_to_available_name(
                self.temporary.path(),
                &self.folder,
                &self.name,
            )?),
            Some(expected) => {
                let target = self.folder.join(&self.name);
                match self.replace(&target, expected)? {
                    ExchangeOutcome::Replaced => target,
                    ExchangeOutcome::Refused => return Ok(Placement::ChangedLocally),
                    ExchangeOutcome::RestoreFailed => {
                        tracing::warn!(path = %self.temporary.path().display(), "failed to restore file edited during download");
                        self.temporary.keep();
                        return Ok(Placement::ChangedLocally);
                    }
                }
            }
        };
        self.temporary.keep();
        sync_folder(&self.folder)?;
        let metadata = std::fs::symlink_metadata(&path)?;
        Ok(Placement::Placed { path, metadata })
    }

    fn replace(&self, target: &Path, expected: &LocalItem) -> io::Result<ExchangeOutcome> {
        let downloaded = self.temporary.path();
        match renameat_with(CWD, downloaded, CWD, target, RenameFlags::EXCHANGE) {
            Ok(()) => {}
            Err(Errno::NOENT) => return Ok(ExchangeOutcome::Refused),
            Err(Errno::INVAL | Errno::NOSYS | Errno::NOTSUP) => {
                if !matches_item(target, expected) {
                    return Ok(ExchangeOutcome::Refused);
                }
                std::fs::rename(downloaded, target)?;
                return Ok(ExchangeOutcome::Replaced);
            }
            Err(errno) => return Err(errno.into()),
        }
        if matches_item(downloaded, expected) {
            if let Err(error) = std::fs::remove_file(downloaded) {
                tracing::warn!(path = %downloaded.display(), %error, "failed to remove replaced file");
            }
            return Ok(ExchangeOutcome::Replaced);
        }
        if renameat_with(CWD, downloaded, CWD, target, RenameFlags::EXCHANGE).is_ok()
            || move_to_available_name(downloaded, &self.folder, &self.name).is_ok()
        {
            return Ok(ExchangeOutcome::Refused);
        }
        Ok(ExchangeOutcome::RestoreFailed)
    }
}

fn matches_item(path: &Path, expected: &LocalItem) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.ino() == expected.id
            && metadata.len() == expected.size
            && modified(&metadata) == expected.modified
    })
}
