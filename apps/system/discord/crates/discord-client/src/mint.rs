use std::sync::atomic::{AtomicU64, Ordering};

use discord_core::MessageId;

#[derive(Debug, Default)]
pub(crate) struct Mint(AtomicU64);

impl Mint {
    pub(crate) fn next(&self, millis: i64) -> MessageId {
        let now = MessageId::minted_at(millis).get();
        let minted = |last: u64| now.max(last + 1);
        let last = self.0.update(Ordering::Relaxed, Ordering::Relaxed, minted);
        MessageId::new(minted(last))
    }
}
