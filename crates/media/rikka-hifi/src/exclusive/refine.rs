use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

const ACCESS: usize = 0;
const FORMAT: usize = 1;
const FIRST_INTERVAL: usize = 8;
const CHANNELS: usize = 10 - FIRST_INTERVAL;
const RATE: usize = 11 - FIRST_INTERVAL;
const RW_INTERLEAVED: u32 = 3;
const OPEN_MIN: u32 = 1;
const OPEN_MAX: u32 = 1 << 1;
const INTEGER: u32 = 1 << 2;
const ATTEMPTS: usize = 3;

const REFINE: u64 = (3 << 30) | ((size_of::<Space>() as u64) << 16) | ((b'A' as u64) << 8) | 0x10;

#[repr(C)]
#[derive(Clone, Copy)]
struct Mask {
    bits: [u32; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Interval {
    min: u32,
    max: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Space {
    _flags: u32,
    masks: [Mask; 3],
    _reserved_masks: [Mask; 5],
    intervals: [Interval; 12],
    _reserved_intervals: [Interval; 9],
    requested: u32,
    _changed: u32,
    _info: u32,
    _msbits: u32,
    _rate_num: u32,
    _rate_den: u32,
    _fifo_size: libc::c_ulong,
    _sync: [u8; 16],
    _reserved: [u8; 48],
}

pub(crate) struct Substream(File);

impl Substream {
    pub(crate) fn open(card: u32, device: u32) -> io::Result<Substream> {
        let path = format!("/dev/snd/pcmC{card}D{device}p");
        let mut appending = false;
        let mut attempts = 0;
        loop {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .append(appending)
                .custom_flags(libc::O_NONBLOCK)
                .open(&path);
            attempts += 1;
            match file {
                Ok(file) => return Ok(Substream(file)),
                Err(error) if attempts < ATTEMPTS => match error.raw_os_error() {
                    Some(libc::EBUSY) => appending = true,
                    Some(libc::EBADFD) => appending = false,
                    _ => return Err(error),
                },
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn interleaved(&self) -> Option<Space> {
        let mut space = Space::any();
        space.masks[ACCESS] = Mask::only(RW_INTERLEAVED);
        self.refined(space)
    }

    pub(crate) fn contains(&self, space: Space) -> bool {
        self.refined(space).is_some()
    }

    fn refined(&self, mut space: Space) -> Option<Space> {
        space.requested = u32::MAX;
        let answer = unsafe {
            libc::ioctl(
                self.0.as_raw_fd(),
                REFINE as libc::Ioctl,
                std::ptr::from_mut(&mut space),
            )
        };
        (answer == 0).then_some(space)
    }
}

impl Space {
    fn any() -> Space {
        let all = Interval {
            min: 0,
            max: u32::MAX,
            flags: 0,
        };
        let unused = Interval {
            min: 0,
            max: 0,
            flags: 0,
        };
        Space {
            _flags: 0,
            masks: [Mask {
                bits: [u32::MAX; 8],
            }; 3],
            _reserved_masks: [Mask { bits: [0; 8] }; 5],
            intervals: [all; 12],
            _reserved_intervals: [unused; 9],
            requested: u32::MAX,
            _changed: 0,
            _info: u32::MAX,
            _msbits: 0,
            _rate_num: 0,
            _rate_den: 0,
            _fifo_size: 0,
            _sync: [0; 16],
            _reserved: [0; 48],
        }
    }

    pub(crate) fn with_format(mut self, format: u32) -> Space {
        self.masks[FORMAT] = Mask::only(format);
        self
    }

    pub(crate) fn with_rate(mut self, rate: u32) -> Space {
        self.intervals[RATE] = Interval::single(rate);
        self
    }

    pub(crate) fn with_channels(mut self, channels: u32) -> Space {
        self.intervals[CHANNELS] = Interval::single(channels);
        self
    }

    pub(crate) fn channels(&self) -> (u32, u32) {
        let channels = self.intervals[CHANNELS];
        let lowest = channels.min + u32::from(channels.flags & OPEN_MIN != 0);
        let highest = channels.max - u32::from(channels.flags & OPEN_MAX != 0);
        (lowest, highest)
    }
}

impl Mask {
    fn only(value: u32) -> Mask {
        let mut bits = [0; 8];
        if let Some(word) = bits.get_mut((value / 32) as usize) {
            *word = 1 << (value % 32);
        }
        Mask { bits }
    }
}

impl Interval {
    fn single(value: u32) -> Interval {
        Interval {
            min: value,
            max: value,
            flags: INTEGER,
        }
    }
}

impl std::fmt::Debug for Substream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Substream").finish_non_exhaustive()
    }
}
