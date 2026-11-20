use crate::sample_format::SampleFormat;

pub const DSD_SILENCE: u8 = 0x69;

const MARKERS: [u8; 2] = [0x05, 0xFA];

#[derive(Debug, Clone)]
pub struct DopPacker {
    channels: usize,
    format: SampleFormat,
    marker: usize,
    carry: Vec<u8>,
}

impl DopPacker {
    #[must_use]
    pub fn new(channels: u16, format: SampleFormat) -> DopPacker {
        assert!(
            format.carries_dop(),
            "DoP needs a 24 or 32-bit integer container, not {format}"
        );
        DopPacker {
            channels: usize::from(channels.max(1)),
            format,
            marker: 0,
            carry: Vec::new(),
        }
    }

    pub fn pack(&mut self, dsd: &[u8], out: &mut Vec<u8>) {
        let frame = self.channels * 2;
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
        self.carry.resize(self.channels * 2, DSD_SILENCE);
        let carried = std::mem::take(&mut self.carry);
        self.frame(&carried, out);
    }

    pub fn reset(&mut self) {
        self.carry.clear();
        self.marker = 0;
    }

    fn frame(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        let marker = MARKERS[self.marker];
        self.marker ^= 1;
        for channel in 0..self.channels {
            let older = bytes[channel];
            let newer = bytes[self.channels + channel];
            match self.format {
                SampleFormat::S32 => out.extend_from_slice(&[0, newer, older, marker]),
                SampleFormat::S24 => out.extend_from_slice(&[newer, older, marker, 0]),
                _ => out.extend_from_slice(&[newer, older, marker]),
            }
        }
    }
}
