use std::ffi::CString;
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use crate::decoding::source::Media;
use crate::error::Result;
use crate::format::AudioFormat;
use crate::reader::{self, StreamReader};

pub(crate) struct Input {
    pub(crate) media: Box<dyn Media>,
    pub(crate) name: Option<CString>,
}

impl Input {
    pub(crate) fn file(path: &Path, format: Option<AudioFormat>) -> Result<Input> {
        let extension = match format {
            Some(format) => Some(format.extension()),
            None => path.extension().and_then(|extension| extension.to_str()),
        };
        Ok(Input {
            media: Box::new(File::open(path)?),
            name: extension.and_then(named),
        })
    }

    pub(crate) fn bytes(bytes: Arc<[u8]>, format: Option<AudioFormat>) -> Input {
        Input {
            media: Box::new(Cursor::new(bytes)),
            name: format.map(AudioFormat::extension).and_then(named),
        }
    }

    pub(crate) fn reader(source: impl StreamReader + 'static) -> Input {
        Input {
            media: reader::media(source),
            name: None,
        }
    }
}

fn named(extension: &str) -> Option<CString> {
    CString::new(format!("sound.{extension}")).ok()
}
