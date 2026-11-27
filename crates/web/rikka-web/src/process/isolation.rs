use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{Mode, OFlags};
use rustix::mount::{MountFlags, MountPropagationFlags};
use rustix::thread::UnshareFlags;

use crate::error::{Error, Result};

const CHROME_POLICIES: &str = "/etc/opt/chrome/policies";

const RIKKA_POLICIES: &str = "/etc/rikka/web/policies";

const APPLICATIONS: &str = "applications";

const PROFILE_APPLICATIONS: &str = "Rikka Applications";

const TMPFS: &CStr = c"tmpfs";

enum Mount {
    Bind { source: CString, target: CString },
    Empty { target: CString },
}

pub(crate) struct Isolation {
    uid_map: Vec<u8>,
    gid_map: Vec<u8>,
    mounts: Vec<Mount>,
}

impl Isolation {
    pub(crate) fn plan(profile: &Path) -> Result<Isolation> {
        let uid = rustix::process::getuid().as_raw();
        let gid = rustix::process::getgid().as_raw();
        let mut mounts = Vec::new();
        mounts.extend(policies()?);
        if let Some(data_home) = rikka_dirs::data_home() {
            let target = data_home.join(APPLICATIONS);
            let source = profile.join(PROFILE_APPLICATIONS);
            for folder in [&target, &source] {
                std::fs::create_dir_all(folder).map_err(|source| Error::Isolation {
                    path: folder.clone(),
                    source,
                })?;
            }
            mounts.push(Mount::Bind {
                source: c_path(&source)?,
                target: c_path(&target)?,
            });
        }
        Ok(Isolation {
            uid_map: format!("{uid} {uid} 1").into_bytes(),
            gid_map: format!("{gid} {gid} 1").into_bytes(),
            mounts,
        })
    }

    pub(crate) fn enter(&self) -> io::Result<()> {
        unsafe { rustix::thread::unshare_unsafe(UnshareFlags::NEWUSER | UnshareFlags::NEWNS) }?;
        write_file(c"/proc/self/setgroups", b"deny")?;
        write_file(c"/proc/self/uid_map", &self.uid_map)?;
        write_file(c"/proc/self/gid_map", &self.gid_map)?;
        rustix::mount::mount_change(
            c"/",
            MountPropagationFlags::REC | MountPropagationFlags::PRIVATE,
        )?;
        for mount in &self.mounts {
            match mount {
                Mount::Bind { source, target } => {
                    rustix::mount::mount_bind(source.as_c_str(), target.as_c_str())?;
                }
                Mount::Empty { target } => rustix::mount::mount(
                    TMPFS,
                    target.as_c_str(),
                    TMPFS,
                    MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC,
                    None,
                )?,
            }
        }
        Ok(())
    }
}

fn policies() -> Result<Option<Mount>> {
    let target = Path::new(CHROME_POLICIES);
    let source = Path::new(RIKKA_POLICIES);
    match (target.is_dir(), source.is_dir()) {
        (true, true) => Ok(Some(Mount::Bind {
            source: c_path(source)?,
            target: c_path(target)?,
        })),
        (true, false) => Ok(Some(Mount::Empty {
            target: c_path(target)?,
        })),
        (false, true) => Err(Error::PolicyMountMissing(target.to_owned())),
        (false, false) => Ok(None),
    }
}

fn c_path(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Isolation {
        path: path.to_owned(),
        source: io::ErrorKind::InvalidInput.into(),
    })
}

fn write_file(path: &CStr, contents: &[u8]) -> io::Result<()> {
    let file = rustix::fs::open(path, OFlags::WRONLY | OFlags::CLOEXEC, Mode::empty())?;
    let written = rustix::io::write(&file, contents)?;
    if written != contents.len() {
        return Err(io::ErrorKind::WriteZero.into());
    }
    Ok(())
}
