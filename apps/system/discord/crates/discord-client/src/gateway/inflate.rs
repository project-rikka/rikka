use flate2::{Decompress, FlushDecompress, Status};

const SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];
const CHUNK: usize = 64 * 1024;

pub(crate) struct Inflater {
    stream: Decompress,
    pending: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
#[error("corrupt compressed gateway frame: {0}")]
pub(crate) struct Corrupt(String);

impl Inflater {
    pub(crate) fn new() -> Inflater {
        Inflater {
            stream: Decompress::new(true),
            pending: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, frame: &[u8]) -> Result<Option<Vec<u8>>, Corrupt> {
        self.pending.extend_from_slice(frame);
        if !self.pending.ends_with(&SUFFIX) {
            return Ok(None);
        }
        let input = std::mem::take(&mut self.pending);
        let mut output = Vec::with_capacity(input.len() * 4);
        let mut consumed = 0;
        loop {
            if output.capacity() - output.len() < CHUNK {
                output.reserve(CHUNK);
            }
            let before_in = self.stream.total_in();
            let before_out = output.len();
            let status = self
                .stream
                .decompress_vec(&input[consumed..], &mut output, FlushDecompress::Sync)
                .map_err(|error| Corrupt(error.to_string()))?;
            let step = usize::try_from(self.stream.total_in() - before_in).unwrap_or(0);
            consumed += step;
            let finished = consumed >= input.len() && output.len() < output.capacity();
            let stalled = step == 0 && output.len() == before_out;
            if finished || stalled || matches!(status, Status::StreamEnd) {
                break;
            }
        }
        Ok(Some(output))
    }
}
