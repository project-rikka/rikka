use std::fmt;
use std::os::fd::OwnedFd;

use smallvec::SmallVec;

use crate::client::handle::Handle;
use crate::client::proxy::Proxy;
use crate::interface::Interface;
use crate::writer::{Fd, fixed_point};

pub(crate) struct Creation {
    pub(crate) offset: usize,
    pub(crate) version: Option<u32>,
}

pub struct Request<'h> {
    pub(crate) handle: &'h Handle,
    pub(crate) opcode: u16,
    pub(crate) body: SmallVec<[u8; 64]>,
    pub(crate) fds: SmallVec<[OwnedFd; 1]>,
    pub(crate) objects: SmallVec<[(u32, u32); 2]>,
    pub(crate) creation: Option<Creation>,
    pub(crate) destructor: bool,
    pub(crate) broken: bool,
}

impl<'h> Request<'h> {
    pub(crate) fn new(handle: &'h Handle, opcode: u16) -> Self {
        Self {
            handle,
            opcode,
            body: SmallVec::new(),
            fds: SmallVec::new(),
            objects: SmallVec::new(),
            creation: None,
            destructor: false,
            broken: false,
        }
    }

    pub fn int(&mut self, value: i32) -> &mut Self {
        self.body.extend_from_slice(&value.to_ne_bytes());
        self
    }

    pub fn uint(&mut self, value: u32) -> &mut Self {
        self.body.extend_from_slice(&value.to_ne_bytes());
        self
    }

    pub fn fixed(&mut self, value: f64) -> &mut Self {
        self.int(fixed_point(value))
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        let bytes = value.as_bytes();
        let content = match bytes.iter().position(|byte| *byte == 0) {
            Some(end) => &bytes[..end],
            None => bytes,
        };
        self.uint((content.len() + 1) as u32);
        self.body.extend_from_slice(content);
        self.body.push(0);
        self.pad(content.len() + 1)
    }

    pub fn nullable_string(&mut self, value: Option<&str>) -> &mut Self {
        match value {
            Some(value) => self.string(value),
            None => self.uint(0),
        }
    }

    pub fn object(&mut self, object: &Handle) -> &mut Self {
        if !object.connection().same(self.handle.connection()) {
            self.broken = true;
        }
        self.objects.push((object.id(), object.generation()));
        self.uint(object.id())
    }

    pub fn nullable_object(&mut self, object: Option<&Handle>) -> &mut Self {
        match object {
            Some(object) => self.object(object),
            None => self.uint(0),
        }
    }

    pub fn array(&mut self, bytes: &[u8]) -> &mut Self {
        self.uint(bytes.len() as u32);
        self.body.extend_from_slice(bytes);
        self.pad(bytes.len())
    }

    pub fn fd<'f>(&mut self, fd: impl Into<Fd<'f>>) -> &mut Self {
        match fd.into() {
            Fd::Owned(fd) => self.fds.push(fd),
            Fd::Borrowed(fd) => match rustix::io::fcntl_dupfd_cloexec(fd, 0) {
                Ok(duplicate) => self.fds.push(duplicate),
                Err(error) => {
                    tracing::warn!("failed to duplicate a descriptor for a request: {error}");
                    self.broken = true;
                }
            },
        }
        self
    }

    pub fn new_id(&mut self) -> &mut Self {
        self.creation = Some(Creation {
            offset: self.body.len(),
            version: None,
        });
        self.uint(0)
    }

    pub fn untyped_new_id(&mut self, interface: &'static Interface, version: u32) -> &mut Self {
        self.string(interface.name).uint(version);
        self.creation = Some(Creation {
            offset: self.body.len(),
            version: Some(version),
        });
        self.uint(0)
    }

    pub fn destroys(&mut self) -> &mut Self {
        self.destructor = true;
        self
    }

    pub fn send(self) {
        self.handle.connection().send(self, None);
    }

    pub fn send_constructor<P: Proxy>(self) -> P {
        let connection = self.handle.connection().clone();
        let created = connection.send(self, Some(P::INTERFACE));
        let handle = created.unwrap_or_else(|| connection.inert(P::INTERFACE));
        P::from_handle(handle).expect("a handle created for P has P's interface")
    }

    fn pad(&mut self, length: usize) -> &mut Self {
        let padding = (4 - length % 4) % 4;
        self.body.extend_from_slice(&[0; 3][..padding]);
        self
    }
}

impl fmt::Debug for Request<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Request")
            .field("object", self.handle)
            .field("opcode", &self.opcode)
            .finish_non_exhaustive()
    }
}
