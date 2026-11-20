use std::io::{self, Write};

use crate::animated::Dispose;
use crate::codecs::gif::lzw::Encoder;
use crate::codecs::gif::quantize::{Indexed, Quantizer};
use crate::codecs::gif::{
    APPLICATION, COLOR_TABLE, DISPOSAL_SHIFT, EXTENSION, GRAPHIC_CONTROL, IMAGE, LOOP_COUNT,
    LOOPING, TRAILER, TRANSPARENT,
};
use crate::loops::Loops;

const SIGNATURE: &[u8; 6] = b"GIF89a";
const SMALLEST_TABLE_BITS: u32 = 1;
const SMALLEST_CODE_BITS: u8 = 2;

#[derive(Debug)]
pub(crate) struct GifStream<W: Write> {
    writer: W,
    width: u16,
    height: u16,
    quantizer: Quantizer,
    indexed: Indexed,
    lzw: Encoder,
    block: Vec<u8>,
}

impl<W: Write> GifStream<W> {
    pub(crate) fn start(mut writer: W, width: u16, height: u16, loops: Loops) -> io::Result<Self> {
        writer.write_all(SIGNATURE)?;
        writer.write_all(&width.to_le_bytes())?;
        writer.write_all(&height.to_le_bytes())?;
        writer.write_all(&[0, 0, 0])?;
        let repeats = match loops {
            Loops::Infinite => Some(0),
            Loops::Finite(0 | 1) => None,
            Loops::Finite(count) => Some(u16::try_from(count).unwrap_or(u16::MAX)),
        };
        if let Some(repeats) = repeats {
            writer.write_all(&[EXTENSION, APPLICATION, LOOPING.len() as u8])?;
            writer.write_all(LOOPING)?;
            let [low, high] = repeats.to_le_bytes();
            writer.write_all(&[3, LOOP_COUNT, low, high, 0])?;
        }
        Ok(Self {
            writer,
            width,
            height,
            quantizer: Quantizer::new(),
            indexed: Indexed::default(),
            lzw: Encoder::new(),
            block: Vec::new(),
        })
    }

    pub(crate) fn frame(
        &mut self,
        rgba: &[u8],
        centiseconds: u16,
        dispose: Dispose,
    ) -> io::Result<()> {
        self.quantizer.quantize(rgba, &mut self.indexed);
        let Indexed {
            palette,
            indices,
            transparent,
        } = &self.indexed;
        let table_bits = palette
            .len()
            .next_power_of_two()
            .trailing_zeros()
            .max(SMALLEST_TABLE_BITS);
        let disposal: u8 = match dispose {
            Dispose::Keep => 1,
            Dispose::Background => 2,
            Dispose::Previous => 3,
        };
        let [delay_low, delay_high] = centiseconds.to_le_bytes();
        self.block.clear();
        self.block.extend_from_slice(&[
            EXTENSION,
            GRAPHIC_CONTROL,
            4,
            disposal << DISPOSAL_SHIFT
                | if transparent.is_some() {
                    TRANSPARENT
                } else {
                    0
                },
            delay_low,
            delay_high,
            transparent.unwrap_or(0),
            0,
            IMAGE,
            0,
            0,
            0,
            0,
        ]);
        self.block.extend_from_slice(&self.width.to_le_bytes());
        self.block.extend_from_slice(&self.height.to_le_bytes());
        self.block.push(COLOR_TABLE | (table_bits - 1) as u8);
        self.block.extend_from_slice(palette.as_flattened());
        self.block.resize(
            self.block.len() + ((1 << table_bits) - palette.len()) * 3,
            0,
        );
        let code_bits = (table_bits as u8).max(SMALLEST_CODE_BITS);
        self.lzw.encode(indices, code_bits, &mut self.block);
        self.writer.write_all(&self.block)
    }

    pub(crate) fn finish(mut self) -> io::Result<W> {
        self.writer.write_all(&[TRAILER])?;
        Ok(self.writer)
    }
}
