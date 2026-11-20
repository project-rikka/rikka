use rikka_gpu::ComputeBuffer;
use rikka_gpu::{Compute, Result};

const SPARE: usize = 2;

#[derive(Debug)]
pub struct Ring {
    size: u64,
    buffers: Vec<ComputeBuffer>,
}

impl Ring {
    #[must_use]
    pub fn new(size: u64) -> Ring {
        Ring {
            size: size.max(16),
            buffers: Vec::new(),
        }
    }

    pub fn acquire(&mut self, compute: &Compute) -> Result<ComputeBuffer> {
        let mut spare = 0;
        self.buffers.retain(|buffer| {
            if buffer.is_unique() {
                spare += 1;
                spare <= SPARE
            } else {
                true
            }
        });
        if let Some(free) = self.buffers.iter().find(|buffer| buffer.is_unique()) {
            return Ok(free.clone());
        }
        let made = compute.buffer(self.size)?;
        self.buffers.push(made.clone());
        Ok(made)
    }
}
