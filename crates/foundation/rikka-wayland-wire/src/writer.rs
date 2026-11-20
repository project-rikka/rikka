use std::cell::RefMut;
use std::fmt;
use std::os::fd::{BorrowedFd, OwnedFd};

use crate::error::{Result, WireError};
use crate::ids::{ClientId, MAX_MESSAGE_SIZE};
use crate::object::Object;
use crate::outgoing::{OUTGOING_LIMIT, Outgoing};

#[derive(Debug)]
pub enum Fd<'a> {
    Borrowed(BorrowedFd<'a>),
    Owned(OwnedFd),
}

impl<'a> From<BorrowedFd<'a>> for Fd<'a> {
    fn from(fd: BorrowedFd<'a>) -> Self {
        Self::Borrowed(fd)
    }
}

impl From<OwnedFd> for Fd<'_> {
    fn from(fd: OwnedFd) -> Self {
        Self::Owned(fd)
    }
}

pub struct Writer<'c> {
    outgoing: RefMut<'c, Outgoing>,
    start: usize,
    fd_count: usize,
    object: u32,
    opcode: u16,
    owner: Option<ClientId>,
    error: Option<WireError>,
    finished: bool,
}

impl<'c> Writer<'c> {
    pub(crate) fn new(mut outgoing: RefMut<'c, Outgoing>, object: u32, opcode: u16) -> Self {
        let start = outgoing.len();
        let fd_count = outgoing.fd_count();
        outgoing.extend(&[0; 8]);
        Self {
            outgoing,
            start,
            fd_count,
            object,
            opcode,
            owner: None,
            error: None,
            finished: false,
        }
    }

    pub(crate) fn owned_by(mut self, client: ClientId) -> Self {
        self.owner = Some(client);
        self
    }

    pub(crate) fn opcode(&self) -> u16 {
        self.opcode
    }

    pub fn int(&mut self, value: i32) -> &mut Self {
        self.outgoing.extend(&value.to_ne_bytes());
        self
    }

    pub fn uint(&mut self, value: u32) -> &mut Self {
        self.outgoing.extend(&value.to_ne_bytes());
        self
    }

    pub fn fixed(&mut self, value: f64) -> &mut Self {
        self.int(fixed_point(value))
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        let content = match value.as_bytes().iter().position(|byte| *byte == 0) {
            Some(end) => &value.as_bytes()[..end],
            None => value.as_bytes(),
        };
        self.uint((content.len() + 1) as u32);
        self.outgoing.extend(content);
        self.outgoing.extend(&[0]);
        self.pad(content.len() + 1)
    }

    pub fn nullable_string(&mut self, value: Option<&str>) -> &mut Self {
        match value {
            Some(value) => self.string(value),
            None => self.uint(0),
        }
    }

    pub fn object(&mut self, object: &Object) -> &mut Self {
        let id = self.check(object);
        self.uint(id)
    }

    pub fn nullable_object(&mut self, object: Option<&Object>) -> &mut Self {
        match object {
            Some(object) => self.object(object),
            None => self.uint(0),
        }
    }

    pub fn object_id(&mut self, id: u32) -> &mut Self {
        self.uint(id)
    }

    pub fn new_id(&mut self, object: &Object) -> &mut Self {
        self.object(object)
    }

    pub fn array(&mut self, bytes: &[u8]) -> &mut Self {
        self.uint(bytes.len() as u32);
        self.outgoing.extend(bytes);
        self.pad(bytes.len())
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.outgoing.extend(bytes);
        self
    }

    pub fn fd<'f>(&mut self, fd: impl Into<Fd<'f>>) -> &mut Self {
        match fd.into() {
            Fd::Owned(fd) => self.outgoing.attach(self.start, fd),
            Fd::Borrowed(fd) => match rustix::io::fcntl_dupfd_cloexec(fd, 0) {
                Ok(duplicate) => self.outgoing.attach(self.start, duplicate),
                Err(error) => self.fail(WireError::Io(error.into())),
            },
        }
        self
    }

    pub fn finish(mut self) -> Result<()> {
        self.finished = true;
        let size = self.outgoing.len() - self.start;
        let result = match self.error.take() {
            Some(error) => Err(error),
            None if size > MAX_MESSAGE_SIZE => Err(WireError::BadSize(size)),
            None if self.outgoing.pending() > OUTGOING_LIMIT => Err(WireError::Backlog),
            None => {
                let start = self.start;
                self.outgoing.patch(start, self.object);
                self.outgoing
                    .patch(start + 4, ((size as u32) << 16) | u32::from(self.opcode));
                return Ok(());
            }
        };
        let (start, fd_count) = (self.start, self.fd_count);
        self.outgoing.rollback(start, fd_count);
        result
    }

    fn check(&mut self, object: &Object) -> u32 {
        if !object.alive() {
            self.fail(WireError::DeadObject);
        } else if self.owner.is_some_and(|owner| owner != object.client()) {
            self.fail(WireError::ForeignObject);
        }
        object.id()
    }

    fn pad(&mut self, length: usize) -> &mut Self {
        let padding = (4 - length % 4) % 4;
        self.outgoing.extend(&[0; 3][..padding]);
        self
    }

    fn fail(&mut self, error: WireError) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }
}

pub(crate) fn fixed_point(value: f64) -> i32 {
    let scaled = value * 256.0;
    let whole = scaled as i64;
    let fraction = scaled - whole as f64;
    let rounded = if fraction >= 0.5 {
        whole.saturating_add(1)
    } else if fraction <= -0.5 {
        whole.saturating_sub(1)
    } else {
        whole
    };
    rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Drop for Writer<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let (start, fd_count) = (self.start, self.fd_count);
            self.outgoing.rollback(start, fd_count);
        }
    }
}

impl fmt::Debug for Writer<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("object", &self.object)
            .field("opcode", &self.opcode)
            .finish_non_exhaustive()
    }
}
