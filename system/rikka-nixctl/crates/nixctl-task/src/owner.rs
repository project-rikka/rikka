use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::Command;

const QUERY: u32 = u32::MAX;

const ROOT: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Owner {
    uid: u32,
    gid: u32,
}

impl Owner {
    pub(crate) fn of(path: &Path) -> io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            uid: metadata.uid(),
            gid: metadata.gid(),
        })
    }

    pub(crate) fn act<T>(self, work: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        let _assumed = Assumed::new(self)?;
        work()
    }

    pub(crate) fn run_as(self, command: &mut Command) -> io::Result<()> {
        if self.uid == ROOT {
            return Ok(());
        }
        let account = rikka_dirs::passwd::lookup(self.uid).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no account owns the configuration (uid {})", self.uid),
            )
        })?;
        command
            .uid(self.uid)
            .gid(self.gid)
            .env("HOME", account.home);
        Ok(())
    }
}

struct Assumed {
    uid: u32,
    gid: u32,
}

impl Assumed {
    fn new(owner: Owner) -> io::Result<Self> {
        let assumed = Self {
            gid: set_fsgid(owner.gid),
            uid: set_fsuid(owner.uid),
        };
        if set_fsgid(QUERY) != owner.gid || set_fsuid(QUERY) != owner.uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "failed to act as the owner {}:{} of the configuration",
                    owner.uid, owner.gid
                ),
            ));
        }
        Ok(assumed)
    }
}

impl Drop for Assumed {
    fn drop(&mut self) {
        set_fsuid(self.uid);
        set_fsgid(self.gid);
    }
}

fn set_fsuid(uid: u32) -> u32 {
    unsafe { libc::setfsuid(uid) as u32 }
}

fn set_fsgid(gid: u32) -> u32 {
    unsafe { libc::setfsgid(gid) as u32 }
}
