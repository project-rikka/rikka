use super::context::RawBuffer;

pub(crate) unsafe trait Plain: Copy {}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Line([u8; 64]);

#[derive(Default)]
pub(crate) struct WideBuffer {
    lines: Vec<Line>,
}

impl WideBuffer {
    pub(crate) fn reserve(&mut self, bytes: usize) -> RawBuffer {
        let lines = bytes.div_ceil(64).max(1);
        if self.lines.len() < lines {
            self.lines.resize(lines, Line([0; 64]));
        }
        RawBuffer::new(self.lines.as_mut_ptr().cast::<u8>(), self.lines.len() * 64)
    }
}

impl std::fmt::Debug for WideBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WideBuffer({} bytes)", self.lines.len() * 64)
    }
}
