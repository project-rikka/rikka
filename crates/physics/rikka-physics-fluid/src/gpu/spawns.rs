use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};

use crate::error::Result;

#[derive(Default)]
pub(crate) struct SpawnRecords {
    buffer: Option<ComputeBuffer>,
}

impl SpawnRecords {
    pub(crate) fn hold(
        &mut self,
        compute: &Compute,
        batch: &mut Batch,
        bytes: &[u8],
    ) -> Result<ComputeBuffer> {
        let size = bytes.len() as u64;
        let buffer = match self.buffer.take() {
            Some(buffer) if buffer.size() >= size => buffer,
            _ => compute.buffer(size.next_power_of_two())?,
        };
        batch.write(&buffer, 0, bytes);
        self.buffer = Some(buffer.clone());
        Ok(buffer)
    }
}
