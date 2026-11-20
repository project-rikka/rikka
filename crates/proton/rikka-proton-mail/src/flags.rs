use std::ops::{BitOr, BitOrAssign};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageFlags(u64);

impl MessageFlags {
    pub const RECEIVED: MessageFlags = MessageFlags(1 << 0);
    pub const SENT: MessageFlags = MessageFlags(1 << 1);
    pub const INTERNAL: MessageFlags = MessageFlags(1 << 2);
    pub const END_TO_END: MessageFlags = MessageFlags(1 << 3);
    pub const AUTO: MessageFlags = MessageFlags(1 << 4);
    pub const REPLIED: MessageFlags = MessageFlags(1 << 5);
    pub const REPLIED_ALL: MessageFlags = MessageFlags(1 << 6);
    pub const FORWARDED: MessageFlags = MessageFlags(1 << 7);
    pub const AUTO_REPLIED: MessageFlags = MessageFlags(1 << 8);
    pub const IMPORTED: MessageFlags = MessageFlags(1 << 9);
    pub const OPENED: MessageFlags = MessageFlags(1 << 10);
    pub const RECEIPT_SENT: MessageFlags = MessageFlags(1 << 11);
    pub const NOTIFIED: MessageFlags = MessageFlags(1 << 12);
    pub const TOUCHED: MessageFlags = MessageFlags(1 << 13);
    pub const RECEIPT: MessageFlags = MessageFlags(1 << 14);
    pub const RECEIPT_REQUEST: MessageFlags = MessageFlags(1 << 16);
    pub const PUBLIC_KEY: MessageFlags = MessageFlags(1 << 17);
    pub const SIGN: MessageFlags = MessageFlags(1 << 18);
    pub const UNSUBSCRIBED: MessageFlags = MessageFlags(1 << 19);
    pub const SCHEDULED_SEND: MessageFlags = MessageFlags(1 << 20);
    pub const ALIAS: MessageFlags = MessageFlags(1 << 21);
    pub const DMARC_PASS: MessageFlags = MessageFlags(1 << 23);
    pub const SPF_FAIL: MessageFlags = MessageFlags(1 << 24);
    pub const DKIM_FAIL: MessageFlags = MessageFlags(1 << 25);
    pub const DMARC_FAIL: MessageFlags = MessageFlags(1 << 26);
    pub const HAM_MANUAL: MessageFlags = MessageFlags(1 << 27);
    pub const SPAM_AUTO: MessageFlags = MessageFlags(1 << 28);
    pub const SPAM_MANUAL: MessageFlags = MessageFlags(1 << 29);
    pub const PHISHING_AUTO: MessageFlags = MessageFlags(1 << 30);
    pub const PHISHING_MANUAL: MessageFlags = MessageFlags(1 << 31);

    #[must_use]
    pub const fn empty() -> MessageFlags {
        MessageFlags(0)
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> MessageFlags {
        MessageFlags(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn intersects(self, flags: MessageFlags) -> bool {
        self.0 & flags.0 != 0
    }

    #[must_use]
    pub const fn contains(self, flags: MessageFlags) -> bool {
        self.0 & flags.0 == flags.0
    }

    #[must_use]
    pub const fn union(self, flags: MessageFlags) -> MessageFlags {
        MessageFlags(self.0 | flags.0)
    }

    #[must_use]
    pub const fn difference(self, flags: MessageFlags) -> MessageFlags {
        MessageFlags(self.0 & !flags.0)
    }

    #[must_use]
    pub const fn is_draft(self) -> bool {
        !self.intersects(MessageFlags::RECEIVED.union(MessageFlags::SENT))
    }
}

impl BitOr for MessageFlags {
    type Output = MessageFlags;

    fn bitor(self, other: MessageFlags) -> MessageFlags {
        self.union(other)
    }
}

impl BitOrAssign for MessageFlags {
    fn bitor_assign(&mut self, other: MessageFlags) {
        *self = self.union(other);
    }
}
