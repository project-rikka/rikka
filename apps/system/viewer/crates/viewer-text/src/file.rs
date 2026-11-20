use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use rikka_encoding::{Decoder, Encoding, UTF_8};

const LIMIT: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct TextFile {
    pub text: String,
    pub encoding: &'static str,
    pub bom: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteReport {
    pub encoding: &'static str,
    pub lossy: bool,
}

impl TextFile {
    pub fn read(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let length = file.metadata()?.len();
        let mut bytes = Vec::with_capacity(usize::try_from(length.min(LIMIT)).unwrap_or_default());
        file.take(LIMIT).read_to_end(&mut bytes)?;

        let truncated = length > LIMIT;
        let decoded = Decoder::new().truncated(truncated).decode(bytes);
        Ok(Self {
            text: decoded.text,
            encoding: decoded.encoding.name(),
            bom: decoded.bom,
            truncated,
        })
    }

    pub fn write(path: &Path, text: &str, encoding: &str, bom: bool) -> io::Result<WriteReport> {
        let requested = Encoding::for_label(encoding.as_bytes()).unwrap_or(UTF_8);
        let output = requested.output_encoding();
        let (encoded, _, lossy) = output.encode(text);
        let mut bytes = Vec::with_capacity(encoded.len() + 3);
        if bom && requested == UTF_8 {
            bytes.extend_from_slice(b"\xEF\xBB\xBF");
        }
        bytes.extend_from_slice(&encoded);
        rikka_fs::replace(path, &bytes)?;
        Ok(WriteReport {
            encoding: output.name(),
            lossy,
        })
    }
}
