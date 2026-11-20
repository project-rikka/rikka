#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobArea {
    name: &'static str,
    limit: u64,
}

impl BlobArea {
    #[must_use]
    pub const fn new(name: &'static str, limit_bytes: u64) -> BlobArea {
        BlobArea {
            name,
            limit: limit_bytes,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }
}
