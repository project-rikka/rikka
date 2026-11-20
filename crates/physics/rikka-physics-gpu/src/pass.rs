use rikka_gpu::Error;
use rikka_gpu::{Batch, ComputeBuffer, Dispatch};

use crate::kernel::NamedKernel;

const PUSH_BYTES: usize = 128;

#[must_use]
#[derive(Debug)]
pub struct Pass<'a> {
    kernel: &'a NamedKernel,
    label: &'static str,
    dispatch: Dispatch,
    push: [u8; PUSH_BYTES],
    pushed: usize,
    empty: bool,
}

impl<'a> Pass<'a> {
    pub fn direct(kernel: &'a NamedKernel, threads: u32, width: u32) -> Pass<'a> {
        let groups = threads.div_ceil(width);
        Pass::launched(kernel, Dispatch::new([groups, 1, 1]), groups == 0)
    }

    pub fn indirect(kernel: &'a NamedKernel, counts: &ComputeBuffer, offset: u64) -> Pass<'a> {
        Pass::launched(kernel, Dispatch::indirect(counts, offset), false)
    }

    fn launched(kernel: &'a NamedKernel, dispatch: Dispatch, empty: bool) -> Pass<'a> {
        Pass {
            kernel,
            label: kernel.name(),
            dispatch,
            push: [0; PUSH_BYTES],
            pushed: 0,
            empty,
        }
    }

    pub fn label(mut self, label: &'static str) -> Pass<'a> {
        self.label = label;
        self
    }

    pub fn read(self, buffer: &ComputeBuffer) -> Pass<'a> {
        self.read_at(buffer, 0)
    }

    pub fn write(self, buffer: &ComputeBuffer) -> Pass<'a> {
        self.write_at(buffer, 0)
    }

    pub fn read_at(self, buffer: &ComputeBuffer, offset: u64) -> Pass<'a> {
        self.bytes(&(buffer.address() + offset).to_le_bytes())
            .reads_through(buffer)
    }

    pub fn write_at(self, buffer: &ComputeBuffer, offset: u64) -> Pass<'a> {
        self.bytes(&(buffer.address() + offset).to_le_bytes())
            .writes_through(buffer)
    }

    pub fn reads_through(mut self, buffer: &ComputeBuffer) -> Pass<'a> {
        self.dispatch = self.dispatch.reads_buffer(buffer);
        self
    }

    pub fn writes_through(mut self, buffer: &ComputeBuffer) -> Pass<'a> {
        self.dispatch = self.dispatch.writes_buffer(buffer);
        self
    }

    pub fn uint(self, value: u32) -> Pass<'a> {
        self.bytes(&value.to_le_bytes())
    }

    pub fn float(self, value: f32) -> Pass<'a> {
        self.bytes(&value.to_le_bytes())
    }

    fn bytes(mut self, bytes: &[u8]) -> Pass<'a> {
        let end = self.pushed + bytes.len();
        if end <= PUSH_BYTES {
            self.push[self.pushed..end].copy_from_slice(bytes);
        }
        self.pushed = end;
        self
    }

    pub fn run(self, batch: &mut Batch) {
        if self.empty {
            return;
        }
        let declared = self.kernel.push_bytes();
        if self.pushed != declared || declared > PUSH_BYTES {
            batch.fail(Error::Invalid(format!(
                "{} is given {} push constant bytes, its kernel declares {declared}",
                self.kernel.name(),
                self.pushed
            )));
            return;
        }
        batch.scope(self.label);
        batch.dispatch(
            self.kernel.kernel(),
            self.dispatch.push(&self.push[..self.pushed]),
        );
    }
}
