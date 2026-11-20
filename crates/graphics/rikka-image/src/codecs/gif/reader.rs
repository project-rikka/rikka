use std::time::Duration;

use crate::animated::Dispose;
use crate::codecs::gif::lzw::{Decoder, MAX_MIN_BITS, Progress};
use crate::codecs::gif::{
    APPLICATION, COLOR_TABLE, DISPOSAL_MASK, DISPOSAL_SHIFT, EXTENSION, GRAPHIC_CONTROL, IMAGE,
    INTERLACED, LOOP_COUNT, LOOPING, TABLE_SIZE, TRAILER, TRANSPARENT,
};
use crate::codecs::{Limits, Probe, Reader, damaged};
use crate::color_type::ColorType;
use crate::error::Result;
use crate::format::Format;
use crate::loops::Loops;

const SIGNATURES: [&[u8; 6]; 2] = [b"GIF89a", b"GIF87a"];
const INTERLACE_PASSES: [(usize, usize); 4] = [(0, 8), (4, 8), (2, 4), (1, 2)];

#[derive(Clone, Copy, Debug, Default)]
struct Control {
    dispose: Dispose,
    transparent: Option<u8>,
    delay: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameInfo {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) delay: Duration,
    pub(crate) dispose: Dispose,
}

#[derive(Debug)]
pub(crate) struct GifReader<'d> {
    bytes: Reader<'d>,
    width: u32,
    height: u32,
    palette: Option<&'d [u8]>,
    loops: Loops,
    control: Control,
    lzw: Decoder,
    indices: Vec<u8>,
}

fn malformed(problem: &str) -> crate::error::Error {
    damaged(Format::Gif, problem)
}

fn sub_blocks<'d>(bytes: &mut Reader<'d>, mut each: impl FnMut(&'d [u8])) -> Result<()> {
    loop {
        let length = usize::from(bytes.u8()?);
        if length == 0 {
            return Ok(());
        }
        each(bytes.take(length)?);
    }
}

fn color_table<'d>(bytes: &mut Reader<'d>, packed: u8) -> Result<Option<&'d [u8]>> {
    if packed & COLOR_TABLE == 0 {
        return Ok(None);
    }
    let entries = 2usize << (packed & TABLE_SIZE);
    bytes.take(entries * 3).map(Some)
}

impl<'d> GifReader<'d> {
    pub(crate) fn new(data: &'d [u8]) -> Result<Self> {
        let mut bytes = Reader::new(Format::Gif, data);
        let signature: [u8; 6] = bytes.array()?;
        if !SIGNATURES.contains(&&signature) {
            return Err(malformed("missing signature"));
        }
        let width = u32::from(bytes.u16_le()?);
        let height = u32::from(bytes.u16_le()?);
        let packed = bytes.u8()?;
        bytes.skip(2)?;
        let palette = color_table(&mut bytes, packed)?;
        Ok(Self {
            bytes,
            width,
            height,
            palette,
            loops: Loops::Finite(1),
            control: Control::default(),
            lzw: Decoder::new(),
            indices: Vec::new(),
        })
    }

    pub(crate) fn probe(&self) -> Probe {
        Probe {
            width: self.width,
            height: self.height,
            color: ColorType::Rgba8,
        }
    }

    pub(crate) fn loops(&self) -> Loops {
        self.loops
    }

    pub(crate) fn next_frame(
        &mut self,
        limits: &Limits,
        rgba: &mut Vec<u8>,
    ) -> Result<Option<FrameInfo>> {
        loop {
            if self.bytes.remaining() == 0 {
                return Ok(None);
            }
            match self.bytes.u8()? {
                TRAILER => return Ok(None),
                EXTENSION => self.extension()?,
                IMAGE => return self.image(limits, rgba).map(Some),
                _ => return Err(malformed("unknown block")),
            }
        }
    }

    fn extension(&mut self) -> Result<()> {
        match self.bytes.u8()? {
            GRAPHIC_CONTROL => {
                let mut control = None;
                sub_blocks(&mut self.bytes, |block| {
                    if let [packed, low, high, index] = *block {
                        let centiseconds = u64::from(u16::from_le_bytes([low, high]));
                        control = Some(Control {
                            dispose: match (packed >> DISPOSAL_SHIFT) & DISPOSAL_MASK {
                                2 => Dispose::Background,
                                3 => Dispose::Previous,
                                _ => Dispose::Keep,
                            },
                            transparent: (packed & TRANSPARENT != 0).then_some(index),
                            delay: Duration::from_millis(centiseconds * 10),
                        });
                    }
                })?;
                self.control = control.unwrap_or_default();
            }
            APPLICATION => {
                let mut looping = false;
                let mut loops = None;
                let mut first = true;
                sub_blocks(&mut self.bytes, |block| {
                    if std::mem::take(&mut first) {
                        looping = block == LOOPING;
                    } else if let [LOOP_COUNT, low, high] = *block
                        && looping
                    {
                        loops = Some(match u16::from_le_bytes([low, high]) {
                            0 => Loops::Infinite,
                            count => Loops::Finite(u32::from(count)),
                        });
                    }
                })?;
                self.loops = loops.unwrap_or(self.loops);
            }
            _ => sub_blocks(&mut self.bytes, |_| {})?,
        }
        Ok(())
    }

    fn image(&mut self, limits: &Limits, rgba: &mut Vec<u8>) -> Result<FrameInfo> {
        let left = u32::from(self.bytes.u16_le()?);
        let top = u32::from(self.bytes.u16_le()?);
        let width = u32::from(self.bytes.u16_le()?);
        let height = u32::from(self.bytes.u16_le()?);
        let packed = self.bytes.u8()?;
        let palette = color_table(&mut self.bytes, packed)?
            .or(self.palette)
            .ok_or_else(|| malformed("frame without a colour table"))?;
        let min_bits = self.bytes.u8()?;
        if !(1..=MAX_MIN_BITS).contains(&min_bits) {
            return Err(malformed("invalid code size"));
        }
        let bytes = limits.admit(width, height, ColorType::Rgba8)?;
        let pixels = bytes / 4;
        let control = std::mem::take(&mut self.control);

        let Self {
            bytes: stream,
            lzw,
            indices,
            ..
        } = self;
        lzw.start(min_bits);
        indices.clear();
        indices.reserve(pixels);
        let mut progress = Progress::Hungry;
        sub_blocks(stream, |block| {
            if progress == Progress::Hungry {
                progress = lzw.feed(block, indices, pixels);
            }
        })?;
        if progress == Progress::Malformed {
            return Err(malformed("damaged image data"));
        }

        rgba.clear();
        rgba.resize(bytes, 0);
        let colors = palette.as_chunks::<3>().0;
        let columns = width as usize;
        let rows = height as usize;
        let mut lines = indices.chunks_exact(columns);
        let mut paint = |row: usize, line: &[u8]| {
            let targets = rgba[row * columns * 4..(row + 1) * columns * 4]
                .as_chunks_mut::<4>()
                .0;
            for (target, &index) in targets.iter_mut().zip(line) {
                if control.transparent == Some(index) {
                    continue;
                }
                if let Some(&[red, green, blue]) = colors.get(usize::from(index)) {
                    *target = [red, green, blue, 255];
                }
            }
        };
        if packed & INTERLACED != 0 {
            for (first, step) in INTERLACE_PASSES {
                for (row, line) in (first..rows).step_by(step).zip(lines.by_ref()) {
                    paint(row, line);
                }
            }
        } else {
            for (row, line) in lines.enumerate() {
                paint(row, line);
            }
        }
        Ok(FrameInfo {
            left,
            top,
            width,
            height,
            delay: control.delay,
            dispose: control.dispose,
        })
    }
}
