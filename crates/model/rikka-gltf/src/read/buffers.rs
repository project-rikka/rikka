use std::sync::Arc;

use crate::document::{Document, Index};
use crate::uri::{self, Uri};
use rikka_model_core::ImportCtx;
use rikka_model_core::{Error, Result};

#[derive(Debug)]
enum BufferData<'a> {
    Slice(&'a [u8]),
    Owned(Arc<[u8]>),
}

impl BufferData<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Slice(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Buffers<'a> {
    buffers: Vec<BufferData<'a>>,
}

impl<'a> Buffers<'a> {
    pub(crate) fn load(
        document: &Document,
        binary: Option<&'a [u8]>,
        ctx: &mut ImportCtx<'_>,
    ) -> Result<Self> {
        let mut buffers = Vec::with_capacity(document.buffers.len());
        for (index, buffer) in document.buffers.iter().enumerate() {
            ctx.checkpoint()?;
            let bytes = match &buffer.uri {
                None => match (index, binary) {
                    (0, Some(binary)) => BufferData::Slice(binary),
                    _ => {
                        return Err(Error::Damaged(format!(
                            "buffer {index} has no uri and no GLB binary chunk"
                        )));
                    }
                },
                Some(named) => match uri::parse(named)? {
                    Uri::Data { bytes, .. } => {
                        ctx.charge(bytes.len() as u64)?;
                        BufferData::Owned(Arc::from(bytes))
                    }
                    Uri::File(path) => BufferData::Owned(ctx.read_resource(&path)?),
                    Uri::Remote(remote) => {
                        return Err(Error::Read {
                            uri: remote,
                            reason: "remote buffers are not fetched".to_owned(),
                        });
                    }
                },
            };
            if bytes.bytes().len() < buffer.byte_length {
                return Err(Error::Damaged(format!(
                    "buffer {index} has {} of {} declared bytes",
                    bytes.bytes().len(),
                    buffer.byte_length
                )));
            }
            buffers.push(bytes);
        }
        Ok(Self { buffers })
    }

    pub(crate) fn view(&self, document: &Document, view: Index) -> Result<&[u8]> {
        let described = document
            .buffer_views
            .get(view as usize)
            .ok_or_else(|| Error::Damaged(format!("no buffer view {view}")))?;
        let bytes = self
            .buffers
            .get(described.buffer as usize)
            .ok_or_else(|| {
                Error::Damaged(format!(
                    "buffer view {view} references missing buffer {}",
                    described.buffer
                ))
            })?
            .bytes();
        let end = described.byte_offset.saturating_add(described.byte_length);
        bytes.get(described.byte_offset..end).ok_or_else(|| {
            Error::Damaged(format!(
                "buffer view {view} range {}..{end} exceeds buffer of {} bytes",
                described.byte_offset,
                bytes.len()
            ))
        })
    }
}
