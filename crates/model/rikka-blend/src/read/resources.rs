use std::sync::Arc;

use rikka_model_core::{ImportCtx, Result, Warning};

pub(crate) trait Resources {
    fn max_file_bytes(&self) -> u64;

    fn charge(&mut self, bytes: u64) -> Result<()>;

    fn read_resource(&mut self, uri: &str) -> Result<Arc<[u8]>>;

    fn read_optional_resource(&mut self, uri: &str) -> Result<Option<Arc<[u8]>>>;

    fn warn(&mut self, warning: Warning);
}

impl Resources for ImportCtx<'_> {
    fn max_file_bytes(&self) -> u64 {
        self.options().limits.max_file_bytes
    }

    fn charge(&mut self, bytes: u64) -> Result<()> {
        ImportCtx::charge(self, bytes)
    }

    fn read_resource(&mut self, uri: &str) -> Result<Arc<[u8]>> {
        ImportCtx::read_resource(self, uri)
    }

    fn read_optional_resource(&mut self, uri: &str) -> Result<Option<Arc<[u8]>>> {
        ImportCtx::read_optional_resource(self, uri)
    }

    fn warn(&mut self, warning: Warning) {
        ImportCtx::warn(self, warning);
    }
}
