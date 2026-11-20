use rikka_sync::Mutex;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::sync::{Arc, Weak};

use bytemuck::{Pod, Zeroable};

use crate::Fnv1a;

const ALIGN: usize = 16;

#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Chunk([u8; ALIGN]);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BlobError {
    #[error("blob is {actual} bytes, shorter than its {expected}-byte header")]
    Truncated { expected: usize, actual: usize },
    #[error("blob section at byte {offset} with {len} items overruns the blob")]
    SectionOutOfBounds { offset: u32, len: u32 },
    #[error("blob section at byte {offset} is misaligned")]
    Misaligned { offset: u32 },
}

#[repr(C)]
pub struct Section<T> {
    pub offset: u32,
    pub len: u32,
    item: PhantomData<fn() -> T>,
}

impl<T> Section<T> {
    pub const EMPTY: Self = Self {
        offset: 0,
        len: 0,
        item: PhantomData,
    };

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Clone for Section<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Section<T> {}

impl<T> fmt::Debug for Section<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Section({}..+{})", self.offset, self.len)
    }
}

impl<T> PartialEq for Section<T> {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset && self.len == other.len
    }
}

impl<T> Eq for Section<T> {}

unsafe impl<T: 'static> Zeroable for Section<T> {}
unsafe impl<T: 'static> Pod for Section<T> {}

struct Stored {
    chunks: Box<[Chunk]>,
    len: usize,
    hash: u64,
}

impl Stored {
    #[inline]
    fn bytes(&self) -> &[u8] {
        &bytemuck::cast_slice(&self.chunks)[..self.len]
    }
}

pub struct Blob<H> {
    stored: Arc<Stored>,
    header: PhantomData<fn() -> H>,
}

impl<H> Clone for Blob<H> {
    fn clone(&self) -> Self {
        Self {
            stored: self.stored.clone(),
            header: PhantomData,
        }
    }
}

impl<H> fmt::Debug for Blob<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blob")
            .field("bytes", &self.stored.len)
            .field("hash", &format_args!("{:016x}", self.stored.hash))
            .finish()
    }
}

impl<H> PartialEq for Blob<H> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.stored, &other.stored)
            || (self.stored.hash == other.stored.hash
                && self.stored.bytes() == other.stored.bytes())
    }
}

impl<H: Pod> Blob<H> {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlobError> {
        if bytes.len() < size_of::<H>() {
            return Err(BlobError::Truncated {
                expected: size_of::<H>(),
                actual: bytes.len(),
            });
        }
        Ok(Self::store(bytes.to_vec()))
    }

    fn store(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        let mut chunks = vec![Chunk([0; ALIGN]); len.div_ceil(ALIGN).max(1)].into_boxed_slice();
        bytemuck::cast_slice_mut::<Chunk, u8>(&mut chunks)[..len].copy_from_slice(&bytes);
        let hash = content_hash(&bytes);
        Self {
            stored: Arc::new(Stored { chunks, len, hash }),
            header: PhantomData,
        }
    }

    #[inline]
    pub fn header(&self) -> &H {
        bytemuck::from_bytes(&self.stored.bytes()[..size_of::<H>()])
    }

    #[inline]
    pub fn section<T: Pod>(&self, section: Section<T>) -> &[T] {
        match self.try_section(section) {
            Ok(items) => items,
            Err(error) => panic!("{error}"),
        }
    }

    #[inline]
    pub fn try_section<T: Pod>(&self, section: Section<T>) -> Result<&[T], BlobError> {
        if section.len == 0 {
            return Ok(&[]);
        }
        let start = section.offset as usize;
        if !start.is_multiple_of(align_of::<T>()) {
            return Err(BlobError::Misaligned {
                offset: section.offset,
            });
        }
        let end = (section.len as usize)
            .checked_mul(size_of::<T>())
            .and_then(|size| size.checked_add(start))
            .filter(|&end| end <= self.stored.len)
            .ok_or(BlobError::SectionOutOfBounds {
                offset: section.offset,
                len: section.len,
            })?;
        Ok(bytemuck::cast_slice(&self.stored.bytes()[start..end]))
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        self.stored.bytes()
    }

    #[inline]
    pub fn content_hash(&self) -> u64 {
        self.stored.hash
    }

    #[inline]
    pub fn byte_len(&self) -> usize {
        self.stored.len
    }

    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.stored, &other.stored)
    }
}

#[derive(Debug)]
pub struct BlobWriter<H> {
    bytes: Vec<u8>,
    header: PhantomData<fn() -> H>,
}

impl<H: Pod> Default for BlobWriter<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Pod> BlobWriter<H> {
    pub fn new() -> Self {
        let mut writer = Self {
            bytes: vec![0; size_of::<H>()],
            header: PhantomData,
        };
        writer.pad();
        writer
    }

    fn pad(&mut self) {
        let padded = self.bytes.len().next_multiple_of(ALIGN);
        self.bytes.resize(padded, 0);
    }

    pub fn push<T: Pod>(&mut self, items: &[T]) -> Section<T> {
        assert!(
            align_of::<T>() <= ALIGN,
            "blob items align to at most 16 bytes"
        );
        if items.is_empty() {
            return Section::EMPTY;
        }
        let offset = u32::try_from(self.bytes.len()).expect("blob exceeds 4 GiB");
        self.bytes.extend_from_slice(bytemuck::cast_slice(items));
        self.pad();
        Section {
            offset,
            len: u32::try_from(items.len()).expect("blob section exceeds u32 items"),
            item: PhantomData,
        }
    }

    pub fn finish(mut self, header: H) -> Blob<H> {
        self.bytes[..size_of::<H>()].copy_from_slice(bytemuck::bytes_of(&header));
        Blob::store(self.bytes)
    }
}

pub struct BlobCache<H> {
    entries: Mutex<HashMap<u64, Vec<Weak<Stored>>>>,
    header: PhantomData<fn() -> H>,
}

impl<H> fmt::Debug for BlobCache<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self.entries.lock().len();
        f.debug_struct("BlobCache")
            .field("hashes", &entries)
            .finish()
    }
}

impl<H: Pod> Default for BlobCache<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Pod> BlobCache<H> {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            header: PhantomData,
        }
    }

    pub fn intern(&self, blob: Blob<H>) -> Blob<H> {
        let mut entries = self.entries.lock();
        let bucket = entries.entry(blob.content_hash()).or_default();
        bucket.retain(|weak| weak.strong_count() > 0);
        for weak in bucket.iter() {
            if let Some(stored) = weak.upgrade()
                && stored.bytes() == blob.bytes()
            {
                return Blob {
                    stored,
                    header: PhantomData,
                };
            }
        }
        bucket.push(Arc::downgrade(&blob.stored));
        blob
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.lock();
        entries
            .values()
            .map(|bucket| bucket.iter().filter(|weak| weak.strong_count() > 0).count())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn content_hash(bytes: &[u8]) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(bytes);
    hash.finish()
}
