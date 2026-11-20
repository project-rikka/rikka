use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

use rustix::pty::{OpenptFlags, ioctl_tiocgptpeer, openpt, unlockpt};
use rustix::termios::{LocalModes, tcgetattr};

use crate::environment;

const FLAGS: OpenptFlags = OpenptFlags::RDWR
    .union(OpenptFlags::NOCTTY)
    .union(OpenptFlags::CLOEXEC);

#[derive(Debug)]
pub(crate) struct Terminal {
    master: File,
}

impl Terminal {
    pub(crate) fn spawn(program: &str) -> io::Result<(Terminal, Child)> {
        let master = openpt(FLAGS)?;
        unlockpt(&master)?;
        let child = {
            let peer = ioctl_tiocgptpeer(&master, FLAGS)?;
            let mut command = Command::new(program);
            command
                .stdin(Stdio::from(peer.try_clone()?))
                .stdout(Stdio::from(peer.try_clone()?))
                .stderr(Stdio::from(peer));
            environment::pass_locale(&mut command);
            unsafe {
                command.pre_exec(|| {
                    rustix::process::setsid()?;
                    rustix::process::ioctl_tiocsctty(BorrowedFd::borrow_raw(0))?;
                    Ok(())
                });
            }
            command.spawn()?
        };
        Ok((
            Terminal {
                master: File::from(master),
            },
            child,
        ))
    }

    pub(crate) fn output(&self) -> &File {
        &self.master
    }

    pub(crate) fn hides_input(&self) -> io::Result<bool> {
        let modes = tcgetattr(self.master.as_fd())?;
        Ok(!modes.local_modes.contains(LocalModes::ECHO))
    }
}
