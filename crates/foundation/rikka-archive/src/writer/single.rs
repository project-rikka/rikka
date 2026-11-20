use std::io::{Read, Write};

use crate::error::Result;
use crate::level::Level;
use crate::stream::{EncoderStream, StreamCompression, StreamMetadata};
use crate::writer::options::CreateOptions;
use crate::writer::{NewEntry, WriterBackend};

pub(crate) struct SingleWriterBackend {
    sink: Option<Box<dyn Write>>,
    stream: Option<Box<dyn EncoderStream>>,
    compression: StreamCompression,
    level: Level,
}

impl SingleWriterBackend {
    pub(crate) fn new<W: Write + 'static>(
        sink: W,
        compression: StreamCompression,
        options: &CreateOptions<'_>,
    ) -> SingleWriterBackend {
        SingleWriterBackend {
            sink: Some(Box::new(sink)),
            stream: None,
            compression,
            level: options.level,
        }
    }
}

impl WriterBackend for SingleWriterBackend {
    fn add_file(&mut self, entry: &NewEntry, data: &mut dyn Read) -> Result<()> {
        let Some(sink) = self.sink.take() else {
            return Ok(());
        };

        let metadata = StreamMetadata {
            name: Some(entry.name.rsplit('/').next().unwrap_or_default().to_owned()),
            modified: entry.modified,
            size: Some(entry.size),
        };

        let mut stream = self.compression.encoder_with(sink, self.level, &metadata)?;
        std::io::copy(data, &mut stream)?;
        self.stream = Some(stream);
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<()> {
        match self.stream {
            Some(stream) => stream.finish_stream(),
            None => Ok(()),
        }
    }
}
