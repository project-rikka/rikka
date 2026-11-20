use slotmap::new_key_type;

new_key_type! {
    pub struct WindowId;
}

rikka_wayland_wire::slot_keys!(WindowId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotId(u64);

impl SnapshotId {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}
