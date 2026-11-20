use crate::chunk::FCTL;
use crate::error::{Error, Result};
use crate::header::Header;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dispose {
    Keep,
    Background,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Source,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameControl {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    pub delay_numerator: u16,
    pub delay_denominator: u16,
    pub dispose: Dispose,
    pub blend: Blend,
}

impl FrameControl {
    const LEN: usize = 26;

    pub(crate) fn parse(body: &[u8], canvas: &Header) -> Result<Self> {
        let malformed = Error::Chunk(FCTL);
        let body: &[u8; Self::LEN] = body.try_into().map_err(|_| Error::Chunk(FCTL))?;
        let word =
            |at: usize| u32::from_be_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]]);
        let (width, height, left, top) = (word(4), word(8), word(12), word(16));
        let fits = width > 0
            && height > 0
            && u64::from(left) + u64::from(width) <= canvas.width as u64
            && u64::from(top) + u64::from(height) <= canvas.height as u64;
        if !fits {
            return Err(malformed);
        }
        let dispose = match body[24] {
            0 => Dispose::Keep,
            1 => Dispose::Background,
            2 => Dispose::Previous,
            _ => return Err(malformed),
        };
        let blend = match body[25] {
            0 => Blend::Source,
            1 => Blend::Over,
            _ => return Err(malformed),
        };
        Ok(Self {
            left,
            top,
            width,
            height,
            delay_numerator: u16::from_be_bytes([body[20], body[21]]),
            delay_denominator: u16::from_be_bytes([body[22], body[23]]),
            dispose,
            blend,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationFrame {
    pub control: FrameControl,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub plays: u32,
    pub frames: Vec<AnimationFrame>,
}
