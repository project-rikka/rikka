use crate::error::Result;
use crate::read::AudioInfo;

#[derive(Default)]
pub(crate) struct Block {
    pub(crate) start: i64,
    pub(crate) samples: Vec<f32>,
}

pub(crate) trait Backend: Send {
    fn next(&mut self, block: &mut Block) -> Result<bool>;

    fn seek(&mut self, frame: u64) -> Result<()>;
}

pub(crate) struct Opened {
    pub(crate) backend: Box<dyn Backend>,
    pub(crate) info: AudioInfo,
}
