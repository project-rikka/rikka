use crate::common::BLOCK_LEN;

#[derive(Clone, Debug)]
pub(crate) struct Block {
    bands: usize,
    channels: usize,
    data: Vec<[f32; BLOCK_LEN]>,
}

impl Block {
    pub(crate) fn new(bands: usize, channels: usize) -> Block {
        Block {
            bands,
            channels,
            data: vec![[0.0; BLOCK_LEN]; bands * channels],
        }
    }

    pub(crate) fn bands(&self) -> usize {
        self.bands
    }

    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn set_channels(&mut self, channels: usize) {
        self.channels = channels;
        self.data.clear();
        self.data.resize(self.bands * channels, [0.0; BLOCK_LEN]);
    }

    pub(crate) fn view(&self, band: usize, channel: usize) -> &[f32; BLOCK_LEN] {
        &self.data[band * self.channels + channel]
    }

    pub(crate) fn view_mut(&mut self, band: usize, channel: usize) -> &mut [f32; BLOCK_LEN] {
        &mut self.data[band * self.channels + channel]
    }
}
