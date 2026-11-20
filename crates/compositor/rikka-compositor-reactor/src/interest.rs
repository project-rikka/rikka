use rustix::event::epoll::EventFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest {
    pub readable: bool,
    pub writable: bool,
}

impl Interest {
    pub const READABLE: Self = Self {
        readable: true,
        writable: false,
    };
    pub const WRITABLE: Self = Self {
        readable: false,
        writable: true,
    };
    pub const BOTH: Self = Self {
        readable: true,
        writable: true,
    };

    pub(crate) fn flags(self) -> EventFlags {
        let mut flags = EventFlags::empty();
        if self.readable {
            flags |= EventFlags::IN;
        }
        if self.writable {
            flags |= EventFlags::OUT;
        }
        flags
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready {
    pub readable: bool,
    pub writable: bool,
    pub hangup: bool,
    pub error: bool,
}

impl Ready {
    pub(crate) fn from_flags(flags: EventFlags) -> Self {
        Self {
            readable: flags.contains(EventFlags::IN),
            writable: flags.contains(EventFlags::OUT),
            hangup: flags.intersects(EventFlags::HUP | EventFlags::RDHUP),
            error: flags.contains(EventFlags::ERR),
        }
    }
}
