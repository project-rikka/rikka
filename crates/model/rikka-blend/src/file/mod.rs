mod compression;
mod header;
mod record;
mod sdna;
mod source;

use std::cell::OnceCell;

use rustc_hash::FxHashMap;

use rikka_model_core::{ByteReader, Endian, Error, Result};

pub(crate) use compression::{Packing, packing, peek};
pub(crate) use header::MAGIC;
pub(crate) use record::{Identity, Record, Table, f32s, i16s, i32s};
pub(crate) use sdna::{Layout, Sdna};
pub(crate) use source::Source;

use header::{BHeadLayout, Header};

const NO_OWNER: u32 = u32::MAX;
const PLACEHOLDER: [u8; 4] = *b"ID\0\0";
const LIBRARY: [u8; 4] = *b"LI\0\0";
const LINK_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Block {
    pub(crate) code: [u8; 4],
    pub(crate) layout: u32,
    pub(crate) old: u64,
    start: usize,
    length: usize,
    pub(crate) count: usize,
}

impl Block {
    pub(crate) fn is_id(&self) -> bool {
        !matches!(
            &self.code,
            b"DATA" | b"GLOB" | b"DNA1" | b"REND" | b"TEST" | b"USER" | b"ENDB"
        )
    }

    pub(crate) fn id_code(&self) -> [u8; 2] {
        [self.code[0], self.code[1]]
    }
}

pub(crate) struct LibraryFile {
    pub(crate) uri: String,
    pub(crate) packed: Option<Vec<u8>>,
}

pub(crate) struct Blend<'a> {
    bytes: Source<'a>,
    header: Header,
    pub(crate) sdna: Sdna,
    blocks: Vec<Block>,
    owners: Vec<u32>,
    ids: FxHashMap<u64, u32>,
    data: FxHashMap<(u32, u64), u32>,
    subversion: u32,
    folder: String,
    uri: String,
    libraries: FxHashMap<u32, u32>,
    names: OnceCell<FxHashMap<String, u32>>,
    links: OnceCell<&'a [Blend<'a>]>,
}

impl<'a> Blend<'a> {
    pub(crate) fn open(source: Source<'a>, limit: u64) -> Result<Self> {
        let bytes = compression::unpack(source, limit)?;
        let header = header::decode(&bytes)?;
        let blocks = heads(&bytes, &header)?;

        let dna = blocks
            .iter()
            .find(|block| &block.code == b"DNA1")
            .ok_or_else(|| Error::Damaged("missing DNA1 block".to_owned()))?;
        let sdna = Sdna::parse(
            &bytes[dna.start..dna.start + dna.length],
            header.endian,
            header.pointer_size,
        )?;

        let mut owners = Vec::with_capacity(blocks.len());
        let mut ids = FxHashMap::default();
        let mut data = FxHashMap::default();
        let mut owner = NO_OWNER;
        let mut subversion = 0;
        let mut library = None;
        let mut libraries = FxHashMap::default();
        for (index, block) in blocks.iter().enumerate() {
            let index = index as u32;
            match &block.code {
                b"DATA" => {
                    if owner != NO_OWNER {
                        data.entry((owner, block.old)).or_insert(index);
                    }
                }
                b"GLOB" => {
                    owner = NO_OWNER;
                    subversion = bytes[block.start..block.start + block.length.min(4)]
                        .iter()
                        .take_while(|byte| byte.is_ascii_digit())
                        .fold(0, |value, &byte| value * 10 + u32::from(byte - b'0'));
                }
                code if block.is_id() => {
                    match *code {
                        LIBRARY => library = Some(index),
                        PLACEHOLDER => {
                            if let Some(library) = library {
                                libraries.insert(index, library);
                            }
                        }
                        _ => {}
                    }
                    owner = index;
                    ids.entry(block.old).or_insert(index);
                }
                _ => owner = NO_OWNER,
            }
            owners.push(owner);
        }
        Ok(Self {
            bytes,
            header,
            sdna,
            blocks,
            owners,
            ids,
            data,
            subversion,
            folder: String::new(),
            uri: String::new(),
            libraries,
            names: OnceCell::new(),
            links: OnceCell::new(),
        })
    }

    pub(crate) fn with_uri(mut self, uri: String) -> Self {
        self.folder = match uri.rfind('/') {
            Some(slash) => uri[..=slash].to_owned(),
            None => String::new(),
        };
        self.uri = uri;
        self
    }

    pub(crate) fn link(&self, world: &'a [Blend<'a>]) {
        let _ = self.links.set(world);
    }

    pub(crate) fn folder(&self) -> &str {
        &self.folder
    }

    pub(crate) fn uri(&self) -> &str {
        &self.uri
    }

    pub(crate) fn version(&self) -> u32 {
        self.header.version
    }

    pub(crate) fn at_least(&self, version: u32, subversion: u32) -> bool {
        self.header.version > version
            || (self.header.version == version && self.subversion >= subversion)
    }

    pub(crate) fn endian(&self) -> Endian {
        self.header.endian
    }

    pub(crate) fn pointer_size(&self) -> usize {
        self.header.pointer_size
    }

    pub(crate) fn size(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn bytes_of(&self, block: &Block) -> &[u8] {
        &self.bytes[block.start..block.start + block.length]
    }

    pub(crate) fn libraries(&self) -> Vec<LibraryFile> {
        let mut libraries = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            if block.code != LIBRARY {
                continue;
            }
            let Some(layout) = self.sdna.layouts.get(block.layout as usize) else {
                continue;
            };
            let bytes = self.bytes_of(block);
            let text = |name: &str| {
                layout
                    .field(name)
                    .filter(|field| !field.pointer)
                    .and_then(|field| bytes.get(field.offset..field.offset + field.size))
                    .map(record::c_string)
                    .filter(|text| !text.is_empty())
            };
            let Some(uri) = text("name")
                .or_else(|| text("filepath"))
                .and_then(|path| linked_uri(&self.folder, &path))
            else {
                continue;
            };
            let packed = self.packed_library(index as u32, layout, bytes);
            libraries.push(LibraryFile { uri, packed });
        }
        libraries
    }

    fn pointer_in(&self, layout: &Layout, bytes: &[u8], name: &str) -> Option<u64> {
        let field = layout.field(name).filter(|field| field.pointer)?;
        let slot = bytes.get(field.offset..field.offset + self.pointer_size())?;
        Some(record::address(slot, self.endian())).filter(|&address| address != 0)
    }

    fn packed_library(&self, owner: u32, layout: &Layout, bytes: &[u8]) -> Option<Vec<u8>> {
        let file =
            self.block(self.data_block(owner, self.pointer_in(layout, bytes, "packedfile")?)?);
        let file_layout = self.sdna.layouts.get(file.layout as usize)?;
        let file_bytes = self.bytes_of(file);
        let size_field = file_layout.field("size")?;
        let size = self.endian().u32(
            file_bytes
                .get(size_field.offset..size_field.offset + 4)?
                .try_into()
                .ok()?,
        ) as usize;
        let data =
            self.block(self.data_block(owner, self.pointer_in(file_layout, file_bytes, "data")?)?);
        let data = self.bytes_of(data);
        Some(data[..size.min(data.len())].to_vec())
    }

    pub(crate) fn ids(&'a self, code: [u8; 2]) -> impl Iterator<Item = Record<'a>> {
        self.blocks
            .iter()
            .enumerate()
            .filter(move |(_, block)| block.is_id() && block.id_code() == code)
            .filter_map(|(index, _)| self.record_of(index as u32))
    }

    pub(crate) fn global(&'a self) -> Option<Record<'a>> {
        let index = self
            .blocks
            .iter()
            .position(|block| &block.code == b"GLOB")?;
        self.record_of(index as u32)
    }

    pub(crate) fn id_block(&self, old: u64) -> Option<u32> {
        self.ids.get(&old).copied()
    }

    pub(crate) fn data_block(&self, owner: u32, old: u64) -> Option<u32> {
        self.data.get(&(owner, old)).copied()
    }

    pub(crate) fn block(&self, index: u32) -> &Block {
        &self.blocks[index as usize]
    }

    pub(crate) fn owner_of(&self, index: u32) -> u32 {
        self.owners[index as usize]
    }

    pub(crate) fn address_of(&self, bytes: &[u8]) -> Option<u64> {
        let base = self.bytes.as_ptr() as usize;
        let at = (bytes.as_ptr() as usize).checked_sub(base)?;
        let index = self
            .blocks
            .partition_point(|block| block.start <= at)
            .checked_sub(1)?;
        let block = &self.blocks[index];
        (at < block.start + block.length.max(1)).then(|| block.old + (at - block.start) as u64)
    }

    pub(crate) fn record_of(&'a self, index: u32) -> Option<Record<'a>> {
        self.records_of(index, None).next()
    }

    pub(crate) fn resolved(&'a self, index: u32) -> Option<Record<'a>> {
        self.follow(index, 0)
    }

    fn follow(&'a self, index: u32, depth: usize) -> Option<Record<'a>> {
        if self.block(index).code != PLACEHOLDER {
            return self.record_of(index);
        }
        if depth >= LINK_DEPTH {
            return None;
        }
        let library = self.record_of(*self.libraries.get(&index)?)?;
        let name = self.record_of(index)?.text("name")?;
        let uri = linked_uri(&self.folder, &library_path(&library)?)?;
        let target = self.links.get()?.iter().find(|blend| blend.uri == uri)?;
        let address = *target.names().get(&name)?;
        target.follow(address, depth + 1)
    }

    fn names(&'a self) -> &'a FxHashMap<String, u32> {
        self.names.get_or_init(|| {
            let mut names = FxHashMap::default();
            for (index, block) in self.blocks.iter().enumerate() {
                if !block.is_id() || block.code == PLACEHOLDER {
                    continue;
                }
                if let Some(name) = self
                    .record_of(index as u32)
                    .and_then(|record| record.record("id"))
                    .and_then(|id| id.text("name"))
                {
                    names.entry(name).or_insert(index as u32);
                }
            }
            names
        })
    }

    pub(crate) fn records_of(
        &'a self,
        index: u32,
        expected: Option<usize>,
    ) -> impl Iterator<Item = Record<'a>> {
        let block = self.block(index);
        let owner = match block.is_id() {
            true => index,
            false => self.owner_of(index),
        };
        let layout = match (block.layout, expected) {
            (0, Some(expected)) => Some(expected),
            (declared, _) => Some(declared as usize),
        }
        .and_then(|layout| self.sdna.layouts.get(layout));
        let bytes = self.bytes_of(block);
        let size = layout.map_or(0, |layout| layout.size);
        let count = match size {
            0 => 0,
            size => block.count.min(bytes.len() / size),
        };
        (0..count).filter_map(move |item| {
            let layout = layout?;
            Some(Record::new(
                self,
                owner,
                layout,
                &bytes[item * size..(item + 1) * size],
            ))
        })
    }
}

fn heads(bytes: &[u8], header: &Header) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    let mut reader = ByteReader::with_endian(bytes, header.endian);
    reader.seek(header.length)?;
    let width = match header.heads {
        BHeadLayout::Narrow => 20,
        BHeadLayout::Small => 24,
        BHeadLayout::Large => 32,
    };
    while reader.remaining() >= width {
        let at = reader.position();
        let mut code: [u8; 4] = reader.array()?;
        if header.endian == Endian::Big && code[..2] == [0, 0] {
            code = [code[2], code[3], 0, 0];
        }
        let (length, old, layout, count) = match header.heads {
            BHeadLayout::Narrow => (
                reader.u32()? as usize,
                u64::from(reader.u32()?),
                reader.u32()?,
                reader.u32()? as usize,
            ),
            BHeadLayout::Small => (
                reader.u32()? as usize,
                reader.u64()?,
                reader.u32()?,
                reader.u32()? as usize,
            ),
            BHeadLayout::Large => {
                let layout = reader.u32()?;
                let old = reader.u64()?;
                let length = usize::try_from(reader.u64()?).unwrap_or(usize::MAX);
                let count = usize::try_from(reader.u64()?).unwrap_or(usize::MAX);
                (length, old, layout, count)
            }
        };
        if &code == b"ENDB" {
            break;
        }
        let start = reader.position();
        if length > reader.remaining() {
            return Err(Error::Damaged(format!(
                "block at byte {at} extends {length} bytes past end of file"
            )));
        }
        reader.seek(start + length)?;
        blocks.push(Block {
            code,
            layout,
            old,
            start,
            length,
            count,
        });
    }
    Ok(blocks)
}

fn library_path(library: &Record<'_>) -> Option<String> {
    library
        .text("name")
        .filter(|path| !path.is_empty())
        .or_else(|| library.text("filepath"))
        .filter(|path| !path.is_empty())
}

pub(crate) fn linked_uri(folder: &str, path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let joined = match path.strip_prefix("//") {
        Some(relative) => format!("{folder}{relative}"),
        None if path.starts_with('/') || path.get(1..2) == Some(":") => {
            format!("{folder}{}", path.rsplit('/').next()?)
        }
        None => format!("{folder}{path}"),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}
