use std::collections::VecDeque;

const CAPACITY: usize = 64 * 1024;

#[derive(Debug, Default)]
pub(crate) struct Tail {
    bytes: VecDeque<u8>,
    truncated: bool,
}

impl Tail {
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        let kept = &bytes[bytes.len().saturating_sub(CAPACITY)..];
        let overflow = (self.bytes.len() + kept.len()).saturating_sub(CAPACITY);
        if overflow > 0 || kept.len() < bytes.len() {
            self.truncated = true;
        }
        self.bytes.drain(..overflow);
        self.bytes.extend(kept);
    }

    pub(crate) fn text(&self) -> String {
        let bytes: Vec<u8> = self.bytes.iter().copied().collect();
        let start = match self.truncated {
            true => bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(0, |newline| newline + 1),
            false => 0,
        };
        String::from_utf8_lossy(&bytes[start..]).into_owned()
    }
}
