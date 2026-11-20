use bytes::Bytes;
use tokio::io::AsyncReadExt as _;

use crate::error::Result;

#[derive(Debug)]
pub(super) enum Source {
    Bytes(Bytes),
    File(tokio::fs::File),
}

impl Source {
    pub(super) async fn read(&mut self, size: usize) -> Result<Option<Vec<u8>>> {
        match self {
            Source::Bytes(bytes) if bytes.is_empty() => Ok(None),
            Source::Bytes(bytes) => {
                let taken = size.min(bytes.len());
                Ok(Some(bytes.split_to(taken).to_vec()))
            }
            Source::File(file) => {
                let mut chunk = vec![0u8; size];
                let mut filled = 0;

                while filled < size {
                    let read = file.read(&mut chunk[filled..]).await?;
                    if read == 0 {
                        break;
                    }
                    filled += read;
                }

                chunk.truncate(filled);

                Ok((!chunk.is_empty()).then_some(chunk))
            }
        }
    }
}
