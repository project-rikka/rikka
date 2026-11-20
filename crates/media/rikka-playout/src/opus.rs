use rikka_opus::OpusDecoder;

use crate::playout::Decode;

impl Decode for OpusDecoder {
    fn frames(&self, packet: &[u8]) -> Option<usize> {
        self.samples(packet)
    }

    fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> bool {
        OpusDecoder::decode(self, packet, out)
            .inspect_err(|error| tracing::debug!("failed to decode an Opus packet: {error}"))
            .is_ok()
    }

    fn recover(&mut self, next: &[u8], frames: usize, out: &mut Vec<f32>) -> bool {
        OpusDecoder::recover(self, next, frames, out)
            .inspect_err(|error| tracing::debug!("failed to recover an Opus packet: {error}"))
            .is_ok()
    }

    fn conceal(&mut self, frames: usize, out: &mut Vec<f32>) {
        if let Err(error) = OpusDecoder::conceal(self, frames, out) {
            tracing::debug!("failed to conceal a lost Opus packet: {error}");
            out.extend(std::iter::repeat_n(0.0, frames * self.channels()));
        }
    }
}
