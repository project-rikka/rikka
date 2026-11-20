use std::io;
use std::os::fd::AsFd;

use drm::control::{AtomicCommitFlags, property};
use rustix::io::Errno;
use rustix::ioctl::{Updater, ioctl, opcode};

use crate::drm_fd::DrmFd;

pub(crate) trait Sink {
    fn set(&mut self, object: u32, property: property::Handle, value: u64);
}

#[derive(Debug, Default)]
pub(crate) struct AtomicRequest {
    objects: Vec<u32>,
    counts: Vec<u32>,
    properties: Vec<u32>,
    values: Vec<u64>,
}

#[repr(C)]
struct ModeAtomic {
    flags: u32,
    count_objs: u32,
    objs_ptr: u64,
    count_props_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
    reserved: u64,
    user_data: u64,
}

const MODE_ATOMIC: rustix::ioctl::Opcode = opcode::read_write::<ModeAtomic>(b'd', 0xBC);

const _: () = {
    assert!(size_of::<ModeAtomic>() == 56);
    assert!(std::mem::offset_of!(ModeAtomic, objs_ptr) == 8);
    assert!(std::mem::offset_of!(ModeAtomic, user_data) == 48);
    assert!(MODE_ATOMIC == 0xC038_64BC);
};
impl AtomicRequest {
    pub(crate) fn clear(&mut self) {
        self.objects.clear();
        self.counts.clear();
        self.properties.clear();
        self.values.clear();
    }

    pub(crate) fn submit(&self, fd: &DrmFd, flags: AtomicCommitFlags) -> io::Result<()> {
        loop {
            let mut request = ModeAtomic {
                flags: flags.bits(),
                count_objs: u32::try_from(self.objects.len())
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
                objs_ptr: self.objects.as_ptr() as u64,
                count_props_ptr: self.counts.as_ptr() as u64,
                props_ptr: self.properties.as_ptr() as u64,
                prop_values_ptr: self.values.as_ptr() as u64,
                reserved: 0,
                user_data: 0,
            };
            let updater = unsafe { Updater::<MODE_ATOMIC, ModeAtomic>::new(&mut request) };
            match unsafe { ioctl(fd.as_fd(), updater) } {
                Err(Errno::INTR) => continue,
                result => return result.map_err(io::Error::from),
            }
        }
    }
}

impl Sink for AtomicRequest {
    fn set(&mut self, object: u32, property: property::Handle, value: u64) {
        match (self.objects.last(), self.counts.last_mut()) {
            (Some(last), Some(count)) if *last == object => *count += 1,
            _ => {
                self.objects.push(object);
                self.counts.push(1);
            }
        }
        self.properties.push(property.into());
        self.values.push(value);
    }
}
