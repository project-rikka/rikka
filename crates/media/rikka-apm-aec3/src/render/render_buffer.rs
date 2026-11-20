use crate::block::Block;
use crate::common::{SPECTRUM_LEN, Spectrum};
use crate::fft::FftData;
use crate::ring::Ring;

#[derive(Clone, Debug)]
pub(crate) struct RenderBuffer {
    pub(crate) blocks: Ring<Block>,
    pub(crate) spectra: Ring<Vec<Spectrum>>,
    pub(crate) ffts: Ring<Vec<FftData>>,
}

impl RenderBuffer {
    pub(crate) fn new(len: usize, bands: usize, channels: usize) -> RenderBuffer {
        RenderBuffer {
            blocks: Ring::new(len, Block::new(bands, channels)),
            spectra: Ring::new(len, vec![[0.0; SPECTRUM_LEN]; channels]),
            ffts: Ring::new(len, vec![FftData::default(); channels]),
        }
    }

    pub(crate) fn block(&self, offset: isize) -> &Block {
        &self.blocks.buffer[self.blocks.offset(self.blocks.read, offset)]
    }

    pub(crate) fn spectrum(&self, offset: isize) -> &[Spectrum] {
        &self.spectra.buffer[self.spectra.offset(self.spectra.read, offset)]
    }

    pub(crate) fn fft_buffer(&self) -> &[Vec<FftData>] {
        &self.ffts.buffer
    }

    pub(crate) fn position(&self) -> usize {
        self.ffts.read
    }

    pub(crate) fn spectral_sum(&self, count: usize, sum: &mut Spectrum) {
        *sum = [0.0; SPECTRUM_LEN];
        let mut position = self.spectra.read;
        for _ in 0..count {
            accumulate(&self.spectra.buffer[position], sum);
            position = self.spectra.inc(position);
        }
    }

    pub(crate) fn spectral_sums(
        &self,
        shorter: usize,
        longer: usize,
        sum_shorter: &mut Spectrum,
        sum_longer: &mut Spectrum,
    ) {
        *sum_shorter = [0.0; SPECTRUM_LEN];
        let mut position = self.spectra.read;
        let mut index = 0;
        while index < shorter {
            accumulate(&self.spectra.buffer[position], sum_shorter);
            position = self.spectra.inc(position);
            index += 1;
        }
        *sum_longer = *sum_shorter;
        while index < longer {
            accumulate(&self.spectra.buffer[position], sum_longer);
            position = self.spectra.inc(position);
            index += 1;
        }
    }

    pub(crate) fn headroom(&self) -> isize {
        let (write, read, len) = (
            self.ffts.write as isize,
            self.ffts.read as isize,
            self.ffts.len() as isize,
        );
        if write < read {
            read - write
        } else {
            len - write + read
        }
    }
}

fn accumulate(channels: &[Spectrum], sum: &mut Spectrum) {
    for spectrum in channels {
        for (sum, value) in sum.iter_mut().zip(spectrum) {
            *sum += value;
        }
    }
}
