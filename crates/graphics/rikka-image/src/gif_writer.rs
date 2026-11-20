use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use crate::animated::Dispose;
use crate::codecs::gif::GifStream;
use crate::error::{Error, Result};
use crate::frame::Frame;
use crate::loops::Loops;

const SHORTEST_DELAY_CS: u16 = 2;

#[derive(Debug)]
pub struct GifWriter {
    stream: GifStream<BufWriter<File>>,
    width: u16,
    height: u16,
}

impl GifWriter {
    pub fn create(
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
        loops: Loops,
    ) -> Result<GifWriter> {
        let (Ok(narrow_width), Ok(narrow_height)) = (u16::try_from(width), u16::try_from(height))
        else {
            return Err(Error::InvalidSize { width, height });
        };
        if narrow_width == 0 || narrow_height == 0 {
            return Err(Error::InvalidSize { width, height });
        }
        let file = BufWriter::new(File::create(path.as_ref())?);
        Ok(GifWriter {
            stream: GifStream::start(file, narrow_width, narrow_height, loops)?,
            width: narrow_width,
            height: narrow_height,
        })
    }

    pub fn push(&mut self, frame: Frame) -> Result<()> {
        let (width, height) = frame.image().dimensions();
        if (width, height) != (u32::from(self.width), u32::from(self.height)) {
            return Err(Error::InvalidSize { width, height });
        }
        let delay = centiseconds(frame.delay());
        let pixels = frame.into_image().into_rgba8();
        self.stream.frame(&pixels, delay, Dispose::Background)?;
        Ok(())
    }

    pub fn finish(self) -> Result<()> {
        let mut file = self.stream.finish()?;
        file.flush()?;
        Ok(())
    }
}

fn centiseconds(delay: Duration) -> u16 {
    let rounded = (delay.as_millis() + 5) / 10;
    u16::try_from(rounded)
        .unwrap_or(u16::MAX)
        .max(SHORTEST_DELAY_CS)
}
