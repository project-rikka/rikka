use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::Duration;

use rikka_compositor_reactor::Token;
use rustix::event::{EventfdFlags, eventfd};
use slotmap::SecondaryMap;
use smallvec::SmallVec;

use crate::ids::{BlockerId, SurfaceId};
use crate::syncobj::TimelinePoint;

#[derive(Debug, Default)]
pub(crate) struct Waits {
    fresh: Vec<(BlockerId, OwnedFd)>,
    watching: SecondaryMap<BlockerId, (Token, OwnedFd)>,
    retired: Vec<(Token, OwnedFd)>,
    acquirers: SecondaryMap<SurfaceId, Acquirer>,
    acquiring: SecondaryMap<BlockerId, SurfaceId>,
    unwatched: Vec<SurfaceId>,
    pub(crate) alarm: Option<(Duration, Token)>,
}

#[derive(Debug)]
struct Acquirer {
    fd: OwnedFd,
    token: Option<Token>,
    waiting: SmallVec<[(BlockerId, TimelinePoint); 2]>,
}

#[derive(Debug, Default)]
pub(crate) struct Work {
    pub(crate) fresh: Vec<(BlockerId, OwnedFd)>,
    pub(crate) retired: Vec<(Token, OwnedFd)>,
    pub(crate) unwatched: Vec<SurfaceId>,
}

impl Waits {
    pub(crate) fn readable(&mut self, blocker: BlockerId, fd: OwnedFd) {
        self.fresh.push((blocker, fd));
    }

    pub(crate) fn watching(&mut self, blocker: BlockerId, token: Token, fd: OwnedFd) {
        self.watching.insert(blocker, (token, fd));
    }

    pub(crate) fn acquire(
        &mut self,
        surface: SurfaceId,
        blocker: BlockerId,
        point: TimelinePoint,
    ) -> io::Result<()> {
        if !self.acquirers.contains_key(surface) {
            let fd = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
            self.acquirers.insert(
                surface,
                Acquirer {
                    fd,
                    token: None,
                    waiting: SmallVec::new(),
                },
            );
            self.unwatched.push(surface);
        }
        let acquirer = self
            .acquirers
            .get_mut(surface)
            .ok_or_else(|| io::Error::other("surface destroyed"))?;
        point.notify(acquirer.fd.as_fd())?;
        acquirer.waiting.push((blocker, point));
        self.acquiring.insert(blocker, surface);
        Ok(())
    }

    pub(crate) fn acquirer_fd(&self, surface: SurfaceId) -> Option<BorrowedFd<'_>> {
        let acquirer = self.acquirers.get(surface)?;
        acquirer.token.is_none().then(|| acquirer.fd.as_fd())
    }

    pub(crate) fn acquirer_watched(&mut self, surface: SurfaceId, token: Token) {
        if let Some(acquirer) = self.acquirers.get_mut(surface) {
            acquirer.token = Some(token);
        }
    }

    pub(crate) fn acquired(&mut self, surface: SurfaceId, all: bool) -> SmallVec<[BlockerId; 2]> {
        let mut reached = SmallVec::new();
        let Some(acquirer) = self.acquirers.get_mut(surface) else {
            return reached;
        };
        let mut count = [0u8; 8];
        let _ = rustix::io::read(&acquirer.fd, &mut count);
        acquirer.waiting.retain(|(blocker, point)| {
            let signaled = all || point.signaled();
            if signaled {
                reached.push(*blocker);
            }
            !signaled
        });
        for blocker in &reached {
            self.acquiring.remove(*blocker);
        }
        reached
    }

    pub(crate) fn retire(&mut self, blocker: BlockerId) {
        self.fresh.retain(|(fresh, _)| *fresh != blocker);
        if let Some(watched) = self.watching.remove(blocker) {
            self.retired.push(watched);
        }
        if let Some(surface) = self.acquiring.remove(blocker)
            && let Some(acquirer) = self.acquirers.get_mut(surface)
        {
            acquirer.waiting.retain(|(waiting, _)| *waiting != blocker);
        }
    }

    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        let Some(acquirer) = self.acquirers.remove(surface) else {
            return;
        };
        for (blocker, _) in &acquirer.waiting {
            self.acquiring.remove(*blocker);
        }
        if let Some(token) = acquirer.token {
            self.retired.push((token, acquirer.fd));
        }
    }

    pub(crate) fn take_work(&mut self) -> Work {
        Work {
            fresh: std::mem::take(&mut self.fresh),
            retired: std::mem::take(&mut self.retired),
            unwatched: std::mem::take(&mut self.unwatched),
        }
    }
}
