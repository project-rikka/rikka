mod xbel;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use jiff::Timestamp;

use crate::error::{Error, Result};
use crate::recent::xbel::{Bookmark, Registration, Xbel};

const FILE_NAME: &str = "recently-used.xbel";
const MAX_ENTRIES: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecentApplication {
    name: &'static str,
    exec: &'static str,
}

impl RecentApplication {
    #[must_use]
    pub const fn new(name: &'static str, exec: &'static str) -> RecentApplication {
        RecentApplication { name, exec }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentFile {
    pub path: PathBuf,
    pub mime_type: Option<String>,
    pub used: SystemTime,
}

#[derive(Clone, Debug)]
pub struct RecentFiles {
    list: PathBuf,
}

impl RecentFiles {
    #[must_use]
    pub fn user() -> Option<RecentFiles> {
        user_list().map(RecentFiles::new)
    }

    #[must_use]
    pub fn new(list: impl Into<PathBuf>) -> RecentFiles {
        RecentFiles { list: list.into() }
    }

    pub fn for_application(
        &self,
        application: &RecentApplication,
        limit: usize,
    ) -> Result<Vec<RecentFile>> {
        let xbel = self.read()?;
        let mut files: Vec<RecentFile> = xbel
            .bookmarks
            .iter()
            .filter_map(|bookmark| {
                let registration = bookmark
                    .applications
                    .iter()
                    .find(|registration| registration.name == application.name)?;
                let path = rikka_open::uri_to_path(&bookmark.href)?;
                let used = parse_timestamp(&registration.modified)
                    .or_else(|| parse_timestamp(&bookmark.modified))
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                Some(RecentFile {
                    path,
                    mime_type: bookmark.mime_type.clone(),
                    used,
                })
            })
            .collect();
        files.sort_by_key(|file| std::cmp::Reverse(file.used));
        Ok(files
            .into_iter()
            .filter(|file| file.path.is_file())
            .take(limit)
            .collect())
    }

    pub fn add(&self, file: &Path, mime_type: &str, application: &RecentApplication) -> Result<()> {
        let mut xbel = self.read()?;
        let href = rikka_open::path_to_uri(file);
        let now = Timestamp::now().to_string();
        let at = match xbel
            .bookmarks
            .iter()
            .position(|bookmark| bookmark.href == href)
        {
            Some(at) => at,
            None => {
                xbel.bookmarks.push(Bookmark {
                    href,
                    added: now.clone(),
                    ..Bookmark::default()
                });
                xbel.bookmarks.len() - 1
            }
        };
        let bookmark = &mut xbel.bookmarks[at];
        bookmark.modified.clone_from(&now);
        bookmark.visited.clone_from(&now);
        bookmark.mime_type = Some(mime_type.to_owned());
        match bookmark
            .applications
            .iter_mut()
            .find(|registration| registration.name == application.name)
        {
            Some(registration) => {
                registration.count = registration.count.saturating_add(1);
                registration.modified = now;
                application.exec.clone_into(&mut registration.exec);
            }
            None => bookmark.applications.push(Registration {
                name: application.name.to_owned(),
                exec: application.exec.to_owned(),
                modified: now,
                count: 1,
            }),
        }
        prune(&mut xbel.bookmarks);
        self.write(&xbel)?;
        #[cfg(windows)]
        announce(file);
        Ok(())
    }

    fn read(&self) -> Result<Xbel> {
        match fs::read_to_string(&self.list) {
            Ok(text) => Xbel::parse(&text).map_err(|error| Error::RecentMalformed {
                list: self.list.clone(),
                reason: error.to_string(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Xbel::default()),
            Err(error) => Err(Error::Recent {
                list: self.list.clone(),
                source: error,
            }),
        }
    }

    fn write(&self, xbel: &Xbel) -> Result<()> {
        rikka_fs::Replace::new(&self.list)
            .private()
            .write(xbel.to_string())
            .map_err(|source| Error::Recent {
                list: self.list.clone(),
                source,
            })
    }
}

#[cfg(unix)]
fn user_list() -> Option<PathBuf> {
    rikka_dirs::data_home().map(|home| home.join(FILE_NAME))
}

#[cfg(windows)]
fn user_list() -> Option<PathBuf> {
    Some(
        rikka_dirs::Paths::resolve()
            .ok()?
            .data_dir()
            .join(FILE_NAME),
    )
}

#[cfg(windows)]
fn announce(file: &Path) {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::Shell::{SHARD_PATHW, SHAddToRecentDocs};
    let wide: Vec<u16> = file
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe { SHAddToRecentDocs(SHARD_PATHW.0 as u32, Some(wide.as_ptr().cast())) };
}

fn prune(bookmarks: &mut Vec<Bookmark>) {
    if bookmarks.len() <= MAX_ENTRIES {
        return;
    }
    let mut ages: Vec<SystemTime> = bookmarks
        .iter()
        .map(|bookmark| parse_timestamp(&bookmark.modified).unwrap_or(SystemTime::UNIX_EPOCH))
        .collect();
    ages.sort_unstable_by(|one, other| other.cmp(one));
    let cutoff = ages[MAX_ENTRIES - 1];
    let mut retained = 0;
    bookmarks.retain(|bookmark| {
        let young = parse_timestamp(&bookmark.modified).unwrap_or(SystemTime::UNIX_EPOCH) >= cutoff;
        retained += usize::from(young);
        young && retained <= MAX_ENTRIES
    });
}

fn parse_timestamp(stamp: &str) -> Option<SystemTime> {
    let timestamp: Timestamp = stamp.parse().ok()?;
    Some(SystemTime::from(timestamp))
}
