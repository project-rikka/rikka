use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rustix::fs::{AtFlags, CWD, Timespec, Timestamps, UTIME_OMIT, utimensat};

pub fn set_modified(path: &Path, modified: SystemTime) -> io::Result<()> {
    let times = Timestamps {
        last_access: Timespec {
            tv_sec: 0,
            tv_nsec: UTIME_OMIT,
        },
        last_modification: timespec(modified),
    };
    Ok(utimensat(CWD, path, &times, AtFlags::empty())?)
}

fn timespec(time: SystemTime) -> Timespec {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => Timespec {
            tv_sec: since.as_secs().cast_signed(),
            tv_nsec: since.subsec_nanos().into(),
        },
        Err(before) => {
            let before = before.duration();
            let whole = before.as_secs().cast_signed();
            match before.subsec_nanos() {
                0 => Timespec {
                    tv_sec: -whole,
                    tv_nsec: 0,
                },
                nanos => Timespec {
                    tv_sec: -whole - 1,
                    tv_nsec: (1_000_000_000 - nanos).into(),
                },
            }
        }
    }
}
