use crate::color::Color;
use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::{Error, Result};
use crate::io::{Padding, Reader, Width, Writer};
use crate::key::Key;
use crate::path::PathData;

pub mod id {
    pub const RESOLUTION: u16 = 1005;
    pub const ALPHA_NAMES: u16 = 1006;
    pub const CAPTION: u16 = 1008;
    pub const BACKGROUND_COLOR: u16 = 1010;
    pub const LAYER_STATE: u16 = 1024;
    pub const LAYER_GROUPS: u16 = 1026;
    pub const IPTC: u16 = 1028;
    pub const GRID_AND_GUIDES: u16 = 1032;
    pub const THUMBNAIL_BGR: u16 = 1033;
    pub const COPYRIGHT: u16 = 1034;
    pub const URL: u16 = 1035;
    pub const THUMBNAIL: u16 = 1036;
    pub const GLOBAL_ANGLE: u16 = 1037;
    pub const ICC_PROFILE: u16 = 1039;
    pub const ICC_UNTAGGED: u16 = 1041;
    pub const EFFECTS_VISIBLE: u16 = 1042;
    pub const ID_SEED: u16 = 1044;
    pub const UNICODE_ALPHA_NAMES: u16 = 1045;
    pub const INDEXED_COLOR_COUNT: u16 = 1046;
    pub const TRANSPARENT_INDEX: u16 = 1047;
    pub const GLOBAL_ALTITUDE: u16 = 1049;
    pub const SLICES: u16 = 1050;
    pub const ALPHA_IDENTIFIERS: u16 = 1053;
    pub const URL_LIST: u16 = 1054;
    pub const VERSION_INFO: u16 = 1057;
    pub const EXIF: u16 = 1058;
    pub const EXIF_3: u16 = 1059;
    pub const XMP: u16 = 1060;
    pub const CAPTION_DIGEST: u16 = 1061;
    pub const PRINT_SCALE: u16 = 1062;
    pub const PIXEL_ASPECT_RATIO: u16 = 1064;
    pub const LAYER_COMPS: u16 = 1065;
    pub const LAYER_SELECTION: u16 = 1069;
    pub const LAYER_GROUPS_ENABLED: u16 = 1072;
    pub const MEASUREMENT_SCALE: u16 = 1074;
    pub const TIMELINE: u16 = 1075;
    pub const SHEET_DISCLOSURE: u16 = 1076;
    pub const ONION_SKINS: u16 = 1078;
    pub const COUNT_INFO: u16 = 1080;
    pub const PRINT_INFO: u16 = 1082;
    pub const PRINT_STYLE: u16 = 1083;
    pub const PATH_SELECTION: u16 = 1088;
    pub const FIRST_PATH: u16 = 2000;
    pub const LAST_PATH: u16 = 2997;
    pub const WORK_PATH: u16 = 1025;
    pub const CLIPPING_PATH_NAME: u16 = 2999;
    pub const ORIGIN_PATH: u16 = 3000;
    pub const IMAGE_READY_VARIABLES: u16 = 7000;
    pub const IMAGE_READY_DATA_SETS: u16 = 7001;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolution {
    pub horizontal: f64,
    pub horizontal_unit: u16,
    pub width_unit: u16,
    pub vertical: f64,
    pub vertical_unit: u16,
    pub height_unit: u16,
}

impl Resolution {
    pub fn pixels_per_inch(dpi: f64) -> Self {
        Self {
            horizontal: dpi,
            horizontal_unit: 1,
            width_unit: 1,
            vertical: dpi,
            vertical_unit: 1,
            height_unit: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub position: f64,
    pub direction: GuideDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridAndGuides {
    pub version: u32,
    pub grid_horizontal: u32,
    pub grid_vertical: u32,
    pub guides: Vec<Guide>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailFormat {
    Raw,
    Jpeg,
}

#[derive(Clone, PartialEq)]
pub struct Thumbnail {
    pub format: ThumbnailFormat,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Thumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnail")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl Thumbnail {
    pub fn from_rgb8(width: u32, height: u32, rgb: &[u8]) -> Result<Self> {
        if rgb.len() != width as usize * height as usize * 3 {
            return Err(Error::Planes("thumbnail pixels do not match the size"));
        }
        let stride = Self::stride(width);
        let mut data = vec![0u8; stride * height as usize];
        for (row, source) in data
            .chunks_exact_mut(stride)
            .zip(rgb.chunks_exact(width as usize * 3))
        {
            row[..source.len()].copy_from_slice(source);
        }
        Ok(Self {
            format: ThumbnailFormat::Raw,
            width,
            height,
            data,
        })
    }

    pub fn raw_rgb8(&self) -> Option<Vec<u8>> {
        let stride = Self::stride(self.width);
        let height = self.height as usize;
        if self.format != ThumbnailFormat::Raw || self.data.len() < stride.checked_mul(height)? {
            return None;
        }
        let row = self.width as usize * 3;
        let mut out = Vec::with_capacity(row * height);
        for line in self.data.chunks_exact(stride).take(height) {
            out.extend_from_slice(&line[..row]);
        }
        Some(out)
    }

    fn stride(width: u32) -> usize {
        (width as usize * 24).div_ceil(32) * 4
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
    pub version: u32,
    pub has_real_merged_data: bool,
    pub writer: String,
    pub reader: String,
    pub file_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintScale {
    pub style: i16,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UrlEntry {
    pub kind: Key,
    pub id: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceData {
    Resolution(Resolution),
    AlphaNames(Vec<String>),
    Caption(String),
    BackgroundColor(Color),
    LayerState(u16),
    LayerGroups(Vec<u16>),
    GridAndGuides(GridAndGuides),
    Thumbnail(Thumbnail),
    Copyright(bool),
    Url(String),
    GlobalAngle(i32),
    IccProfile(Vec<u8>),
    IccUntagged(bool),
    EffectsVisible(bool),
    IdSeed(u32),
    UnicodeAlphaNames(Vec<String>),
    IndexedColorCount(u16),
    TransparentIndex(u16),
    GlobalAltitude(i32),
    AlphaIdentifiers(Vec<u32>),
    UrlList(Vec<UrlEntry>),
    VersionInfo(VersionInfo),
    Xmp(String),
    CaptionDigest([u8; 16]),
    PrintScale(PrintScale),
    PixelAspectRatio {
        version: u32,
        ratio: f64,
    },
    LayerSelection(Vec<u32>),
    LayerGroupsEnabled(Vec<u8>),
    Path(PathData),
    Descriptor {
        version: Option<u32>,
        descriptor: Descriptor,
    },
    Raw(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    pub signature: Key,
    pub id: u16,
    pub name: String,
    pub data: ResourceData,
}

impl Resource {
    pub fn new(id: u16, data: ResourceData) -> Self {
        Self {
            signature: Key(*b"8BIM"),
            id,
            name: String::new(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resources {
    pub items: Vec<Resource>,
}

impl Resources {
    pub fn get(&self, id: u16) -> Option<&ResourceData> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .map(|item| &item.data)
    }

    pub fn get_mut(&mut self, id: u16) -> Option<&mut ResourceData> {
        self.items
            .iter_mut()
            .find(|item| item.id == id)
            .map(|item| &mut item.data)
    }

    pub fn set(&mut self, id: u16, data: ResourceData) {
        match self.items.iter_mut().find(|item| item.id == id) {
            Some(item) => item.data = data,
            None => self.items.push(Resource::new(id, data)),
        }
    }

    pub fn remove(&mut self, id: u16) -> Option<Resource> {
        let index = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(index))
    }

    pub fn icc_profile(&self) -> Option<&[u8]> {
        match self.get(id::ICC_PROFILE)? {
            ResourceData::IccProfile(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn resolution(&self) -> Option<&Resolution> {
        match self.get(id::RESOLUTION)? {
            ResourceData::Resolution(resolution) => Some(resolution),
            _ => None,
        }
    }

    pub fn global_angle(&self) -> Option<i32> {
        match self.get(id::GLOBAL_ANGLE)? {
            ResourceData::GlobalAngle(angle) => Some(*angle),
            _ => None,
        }
    }

    pub fn global_altitude(&self) -> Option<i32> {
        match self.get(id::GLOBAL_ALTITUDE)? {
            ResourceData::GlobalAltitude(altitude) => Some(*altitude),
            _ => None,
        }
    }

    pub fn alpha_names(&self) -> Vec<String> {
        match (self.get(id::UNICODE_ALPHA_NAMES), self.get(id::ALPHA_NAMES)) {
            (Some(ResourceData::UnicodeAlphaNames(names)), _)
            | (_, Some(ResourceData::AlphaNames(names))) => names.clone(),
            _ => Vec::new(),
        }
    }

    pub fn thumbnail(&self) -> Option<&Thumbnail> {
        match self
            .get(id::THUMBNAIL)
            .or_else(|| self.get(id::THUMBNAIL_BGR))?
        {
            ResourceData::Thumbnail(thumbnail) => Some(thumbnail),
            _ => None,
        }
    }

    pub fn paths(&self) -> impl Iterator<Item = (&str, &PathData)> {
        self.items.iter().filter_map(|item| match &item.data {
            ResourceData::Path(path) => Some((item.name.as_str(), path)),
            _ => None,
        })
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let mut items = Vec::new();
        while reader.remaining() >= 12 {
            let signature = reader.key()?;
            if !matches!(
                &signature.0,
                b"8BIM" | b"MeSa" | b"AgHg" | b"PHUT" | b"DCSR"
            ) {
                return Err(reader.malformed("image resource signature"));
            }
            let id = reader.u16()?;
            let name = reader.pascal(2)?;
            let length = u64::from(reader.u32()?);
            let mut body = reader.sub(length.min(reader.remaining() as u64))?;
            if length % 2 == 1 && reader.remaining() > 0 {
                reader.skip(1)?;
            }
            let raw = body.rest().to_vec();
            let data = decode(id, &mut body).unwrap_or(ResourceData::Raw(raw));
            items.push(Resource {
                signature,
                id,
                name,
                data,
            });
        }
        Ok(Self { items })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        for item in &self.items {
            writer.key(item.signature);
            writer.u16(item.id);
            writer.pascal(&item.name, 2);
            writer.section(Width::Four, Padding::Trailing(2), |writer| {
                encode(&item.data, writer)
            })?;
        }
        Ok(())
    }
}

fn is_path(id: u16) -> bool {
    (id::FIRST_PATH..=id::LAST_PATH).contains(&id) || id == id::WORK_PATH
}

fn is_descriptor(id: u16) -> bool {
    matches!(
        id,
        id::SLICES
            | id::LAYER_COMPS
            | id::MEASUREMENT_SCALE
            | id::TIMELINE
            | id::SHEET_DISCLOSURE
            | id::ONION_SKINS
            | id::COUNT_INFO
            | id::PRINT_INFO
            | id::PRINT_STYLE
            | id::PATH_SELECTION
            | id::ORIGIN_PATH
            | id::IMAGE_READY_VARIABLES
            | id::IMAGE_READY_DATA_SETS
    )
}

fn decode(id: u16, reader: &mut Reader<'_>) -> Result<ResourceData> {
    let data = match id {
        id::RESOLUTION => ResourceData::Resolution(Resolution {
            horizontal: reader.fixed16()?,
            horizontal_unit: reader.u16()?,
            width_unit: reader.u16()?,
            vertical: reader.fixed16()?,
            vertical_unit: reader.u16()?,
            height_unit: reader.u16()?,
        }),
        id::ALPHA_NAMES => {
            let mut names = Vec::new();
            while !reader.is_empty() {
                names.push(reader.pascal(1)?);
            }
            ResourceData::AlphaNames(names)
        }
        id::CAPTION => ResourceData::Caption(reader.pascal(1)?),
        id::BACKGROUND_COLOR => ResourceData::BackgroundColor(Color::read(reader)?),
        id::LAYER_STATE => ResourceData::LayerState(reader.u16()?),
        id::LAYER_GROUPS => {
            let mut groups = Vec::new();
            while reader.remaining() >= 2 {
                groups.push(reader.u16()?);
            }
            ResourceData::LayerGroups(groups)
        }
        id::GRID_AND_GUIDES => {
            let version = reader.u32()?;
            let grid_horizontal = reader.u32()?;
            let grid_vertical = reader.u32()?;
            let count = reader.u32()?;
            let mut guides = Vec::new();
            for _ in 0..count {
                let position = f64::from(reader.i32()?) / 32.0;
                let direction = if reader.u8()? == 0 {
                    GuideDirection::Vertical
                } else {
                    GuideDirection::Horizontal
                };
                guides.push(Guide {
                    position,
                    direction,
                });
            }
            ResourceData::GridAndGuides(GridAndGuides {
                version,
                grid_horizontal,
                grid_vertical,
                guides,
            })
        }
        id::THUMBNAIL | id::THUMBNAIL_BGR => {
            let format = match reader.u32()? {
                0 => ThumbnailFormat::Raw,
                1 => ThumbnailFormat::Jpeg,
                other => {
                    return Err(Error::Invalid {
                        what: "thumbnail format",
                        value: i64::from(other),
                    });
                }
            };
            let width = reader.u32()?;
            let height = reader.u32()?;
            reader.skip(12)?;
            let bits = reader.u16()?;
            let planes = reader.u16()?;
            if bits != 24 || planes != 1 {
                return Err(reader.malformed("thumbnail"));
            }
            ResourceData::Thumbnail(Thumbnail {
                format,
                width,
                height,
                data: reader.take_rest().to_vec(),
            })
        }
        id::COPYRIGHT => ResourceData::Copyright(reader.bool()?),
        id::URL => ResourceData::Url(String::from_utf8_lossy(reader.take_rest()).into_owned()),
        id::GLOBAL_ANGLE => ResourceData::GlobalAngle(reader.i32()?),
        id::ICC_PROFILE => ResourceData::IccProfile(reader.take_rest().to_vec()),
        id::ICC_UNTAGGED => ResourceData::IccUntagged(reader.bool()?),
        id::EFFECTS_VISIBLE => ResourceData::EffectsVisible(reader.bool()?),
        id::ID_SEED => ResourceData::IdSeed(reader.u32()?),
        id::UNICODE_ALPHA_NAMES => {
            let mut names = Vec::new();
            while reader.remaining() >= 4 {
                names.push(reader.unicode()?);
            }
            ResourceData::UnicodeAlphaNames(names)
        }
        id::INDEXED_COLOR_COUNT => ResourceData::IndexedColorCount(reader.u16()?),
        id::TRANSPARENT_INDEX => ResourceData::TransparentIndex(reader.u16()?),
        id::GLOBAL_ALTITUDE => ResourceData::GlobalAltitude(reader.i32()?),
        id::ALPHA_IDENTIFIERS => {
            let mut ids = Vec::new();
            while reader.remaining() >= 4 {
                ids.push(reader.u32()?);
            }
            ResourceData::AlphaIdentifiers(ids)
        }
        id::URL_LIST => {
            let count = reader.u32()?;
            let mut entries = Vec::new();
            for _ in 0..count {
                entries.push(UrlEntry {
                    kind: reader.key()?,
                    id: reader.u32()?,
                    url: reader.unicode()?,
                });
            }
            ResourceData::UrlList(entries)
        }
        id::VERSION_INFO => ResourceData::VersionInfo(VersionInfo {
            version: reader.u32()?,
            has_real_merged_data: reader.bool()?,
            writer: reader.unicode()?,
            reader: reader.unicode()?,
            file_version: reader.u32()?,
        }),
        id::XMP => ResourceData::Xmp(
            String::from_utf8(reader.take_rest().to_vec()).map_err(|_| reader.malformed("XMP"))?,
        ),
        id::CAPTION_DIGEST => {
            let mut digest = [0u8; 16];
            digest.copy_from_slice(reader.bytes(16)?);
            ResourceData::CaptionDigest(digest)
        }
        id::PRINT_SCALE => ResourceData::PrintScale(PrintScale {
            style: reader.i16()?,
            x: reader.f32()?,
            y: reader.f32()?,
            scale: reader.f32()?,
        }),
        id::PIXEL_ASPECT_RATIO => ResourceData::PixelAspectRatio {
            version: reader.u32()?,
            ratio: reader.f64()?,
        },
        id::LAYER_SELECTION => {
            let count = reader.u16()?;
            let mut ids = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                ids.push(reader.u32()?);
            }
            ResourceData::LayerSelection(ids)
        }
        id::LAYER_GROUPS_ENABLED => ResourceData::LayerGroupsEnabled(reader.take_rest().to_vec()),
        id if is_path(id) => ResourceData::Path(PathData::read(reader)?),
        id if is_descriptor(id) => {
            let first = reader.clone().u32()?;
            if first == 16 {
                ResourceData::Descriptor {
                    version: None,
                    descriptor: read_versioned(reader)?,
                }
            } else {
                let version = reader.u32()?;
                ResourceData::Descriptor {
                    version: Some(version),
                    descriptor: read_versioned(reader)?,
                }
            }
        }
        _ => ResourceData::Raw(reader.take_rest().to_vec()),
    };
    if !reader.finish_padded() {
        return Err(reader.malformed("image resource"));
    }
    Ok(data)
}

fn encode(data: &ResourceData, writer: &mut Writer) -> Result<()> {
    match data {
        ResourceData::Resolution(resolution) => {
            writer.fixed16(resolution.horizontal);
            writer.u16(resolution.horizontal_unit);
            writer.u16(resolution.width_unit);
            writer.fixed16(resolution.vertical);
            writer.u16(resolution.vertical_unit);
            writer.u16(resolution.height_unit);
        }
        ResourceData::AlphaNames(names) => {
            for name in names {
                writer.pascal(name, 1);
            }
        }
        ResourceData::Caption(text) => writer.pascal(text, 1),
        ResourceData::BackgroundColor(color) => color.write(writer),
        ResourceData::LayerState(state) => writer.u16(*state),
        ResourceData::LayerGroups(groups) => groups.iter().for_each(|&group| writer.u16(group)),
        ResourceData::GridAndGuides(grid) => {
            writer.u32(grid.version);
            writer.u32(grid.grid_horizontal);
            writer.u32(grid.grid_vertical);
            writer.u32(grid.guides.len() as u32);
            for guide in &grid.guides {
                writer.i32((guide.position * 32.0).round() as i32);
                writer.u8(u8::from(guide.direction == GuideDirection::Horizontal));
            }
        }
        ResourceData::Thumbnail(thumbnail) => {
            let stride = Thumbnail::stride(thumbnail.width);
            writer.u32(match thumbnail.format {
                ThumbnailFormat::Raw => 0,
                ThumbnailFormat::Jpeg => 1,
            });
            writer.u32(thumbnail.width);
            writer.u32(thumbnail.height);
            writer.u32(stride as u32);
            writer.u32((stride * thumbnail.height as usize) as u32);
            writer.u32(thumbnail.data.len() as u32);
            writer.u16(24);
            writer.u16(1);
            writer.bytes(&thumbnail.data);
        }
        ResourceData::Copyright(flag)
        | ResourceData::IccUntagged(flag)
        | ResourceData::EffectsVisible(flag) => writer.bool(*flag),
        ResourceData::Url(url) => writer.bytes(url.as_bytes()),
        ResourceData::GlobalAngle(value) | ResourceData::GlobalAltitude(value) => {
            writer.i32(*value)
        }
        ResourceData::IccProfile(bytes) | ResourceData::Raw(bytes) => writer.bytes(bytes),
        ResourceData::IdSeed(seed) => writer.u32(*seed),
        ResourceData::UnicodeAlphaNames(names) => names
            .iter()
            .for_each(|name| writer.unicode_terminated(name)),
        ResourceData::IndexedColorCount(value) | ResourceData::TransparentIndex(value) => {
            writer.u16(*value)
        }
        ResourceData::AlphaIdentifiers(ids) => ids.iter().for_each(|&id| writer.u32(id)),
        ResourceData::UrlList(entries) => {
            writer.u32(entries.len() as u32);
            for entry in entries {
                writer.key(entry.kind);
                writer.u32(entry.id);
                writer.unicode(&entry.url);
            }
        }
        ResourceData::VersionInfo(info) => {
            writer.u32(info.version);
            writer.bool(info.has_real_merged_data);
            writer.unicode_terminated(&info.writer);
            writer.unicode_terminated(&info.reader);
            writer.u32(info.file_version);
        }
        ResourceData::Xmp(text) => writer.bytes(text.as_bytes()),
        ResourceData::CaptionDigest(digest) => writer.bytes(digest),
        ResourceData::PrintScale(scale) => {
            writer.i16(scale.style);
            writer.f32(scale.x);
            writer.f32(scale.y);
            writer.f32(scale.scale);
        }
        ResourceData::PixelAspectRatio { version, ratio } => {
            writer.u32(*version);
            writer.f64(*ratio);
        }
        ResourceData::LayerSelection(ids) => {
            writer.u16(u16::try_from(ids.len()).map_err(|_| Error::Oversized)?);
            ids.iter().for_each(|&id| writer.u32(id));
        }
        ResourceData::LayerGroupsEnabled(flags) => writer.bytes(flags),
        ResourceData::Path(path) => path.write(writer),
        ResourceData::Descriptor {
            version,
            descriptor,
        } => {
            if let Some(version) = version {
                writer.u32(*version);
            }
            write_versioned(writer, descriptor)?;
        }
    }
    Ok(())
}
