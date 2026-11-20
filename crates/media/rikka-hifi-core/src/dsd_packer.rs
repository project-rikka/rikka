use crate::dop::DSD_SILENCE;
use crate::sample_format::SampleFormat;

#[derive(Debug, Clone)]
pub struct DsdPacker {
    channels: usize,
    width: usize,
    big_endian: bool,
    carry: Vec<u8>,
}

impl DsdPacker {
    #[must_use]
    pub fn new(channels: u16, format: SampleFormat) -> DsdPacker {
        assert!(format.is_dsd(), "{format} does not carry native DSD");
        DsdPacker {
            channels: usize::from(channels.max(1)),
            width: format.bytes(),
            big_endian: matches!(
                format,
                SampleFormat::Dsd8 | SampleFormat::Dsd16Be | SampleFormat::Dsd32Be
            ),
            carry: Vec::new(),
        }
    }

    pub fn pack(&mut self, dsd: &[u8], out: &mut Vec<u8>) {
        let frame = self.channels * self.width;
        let mut input = dsd;
        if !self.carry.is_empty() {
            let count = (frame - self.carry.len()).min(input.len());
            self.carry.extend_from_slice(&input[..count]);
            input = &input[count..];
            if self.carry.len() < frame {
                return;
            }
            let carried = std::mem::take(&mut self.carry);
            self.frame(&carried, out);
        }
        let mut frames = input.chunks_exact(frame);
        for bytes in frames.by_ref() {
            self.frame(bytes, out);
        }
        self.carry.extend_from_slice(frames.remainder());
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if self.carry.is_empty() {
            return;
        }
        self.carry.resize(self.channels * self.width, DSD_SILENCE);
        let carried = std::mem::take(&mut self.carry);
        self.frame(&carried, out);
    }

    pub fn reset(&mut self) {
        self.carry.clear();
    }

    fn frame(&self, bytes: &[u8], out: &mut Vec<u8>) {
        for channel in 0..self.channels {
            for slot in 0..self.width {
                let age = if self.big_endian {
                    slot
                } else {
                    self.width - 1 - slot
                };
                out.push(bytes[age * self.channels + channel]);
            }
        }
    }
}
