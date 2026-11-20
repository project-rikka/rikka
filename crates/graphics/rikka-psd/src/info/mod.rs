mod adjustment;
mod annotations;
mod descriptor_block;
mod filter;
mod legacy_effects;
mod linked;
mod placed;
mod simple;
mod type_tool;
mod vector_mask;

pub use adjustment::{
    Adjustment, CurveData, Curves, GradientMap, GradientMapColorStop, GradientMapOpacityStop,
    HueSaturation, LevelRecord, Levels, PhotoFilter, PhotoFilterColor,
};
pub use annotations::{Annotation, AnnotationContent, Annotations};
pub use descriptor_block::{DescriptorBlock, Header};
pub use filter::{FilterChannel, FilterEffect, FilterEffects, FilterExtra, FilterMask, UserMask};
pub use legacy_effects::{LegacyEffect, LegacyEffects};
pub use linked::{LinkKind, LinkTime, LinkedFile, LinkedFiles, LinkedKey};
pub use placed::{PlacedKind, PlacedLayer};
pub use simple::{
    Knockout, MetadataItem, MetadataValue, Protection, Section, SectionKind, SheetColor,
};
pub use type_tool::TypeTool;
pub use vector_mask::VectorMask;

use crate::error::Result;
use crate::io::{Padding, Reader, Width, Writer};
use crate::key::Key;
use crate::pattern::Pattern;

#[derive(Debug, Clone, PartialEq)]
pub enum Info {
    UnicodeName(String),
    NameSource(Key),
    Id(u32),
    Version(u32),
    Section(Section),
    SheetColor(SheetColor),
    Knockout(Knockout),
    BlendClipped(bool),
    BlendInterior(bool),
    FillOpacity(u8),
    TransparencyShapes(bool),
    LayerMaskAsGlobal(bool),
    VectorMaskAsGlobal(bool),
    Protection(Protection),
    ReferencePoint { x: f64, y: f64 },
    AlignedRendering(u32),
    ForeignEffectId(u32),
    VectorOriginationFlag(u32),
    ChannelRestrictions(Vec<i32>),
    Metadata(Vec<MetadataItem>),
    VectorMask(VectorMask),
    LegacyEffects(LegacyEffects),
    TypeTool(TypeTool),
    PlacedLayer(PlacedLayer),
    LinkedFiles(LinkedFiles),
    Patterns { key: Key, patterns: Vec<Pattern> },
    Annotations(Annotations),
    FilterMask(FilterMask),
    UserMask(UserMask),
    FilterEffects(FilterEffects),
    Adjustment(Adjustment),
    Descriptor(DescriptorBlock),
    Raw { key: Key, data: Vec<u8> },
}

const WIDE_KEYS: [&[u8; 4]; 23] = [
    b"LMsk", b"Lr16", b"Lr32", b"Layr", b"Mt16", b"Mt32", b"Mtrn", b"Alph", b"FMsk", b"lnk2",
    b"lnk3", b"lnkE", b"FEid", b"FXid", b"FELS", b"PxSD", b"pths", b"extd", b"extn", b"cinf",
    b"artd", b"lnkD", b"shpa",
];

const COUNTED_FOUR: [&[u8; 4]; 26] = [
    b"luni", b"vmsk", b"artb", b"artd", b"vogk", b"SoLd", b"SoLE", b"lnk2", b"lnk3", b"lnkD",
    b"vscg", b"vsms", b"GdFl", b"lmfx", b"lrFX", b"PlLd", b"plLd", b"Anno", b"GenI", b"FEid",
    b"FXid", b"curv", b"CgEd", b"vibA", b"blwh", b"grdm",
];

const TRAILING_FOUR: [&[u8; 4]; 5] = [b"Txt2", b"cinf", b"extn", b"CAI ", b"OCIO"];

pub(crate) fn is_wide(key: Key, psb: bool) -> bool {
    psb && WIDE_KEYS.iter().any(|candidate| **candidate == key.0)
}

fn padding(key: Key) -> Padding {
    if TRAILING_FOUR.iter().any(|candidate| **candidate == key.0) {
        Padding::Trailing(4)
    } else if COUNTED_FOUR.iter().any(|candidate| **candidate == key.0) {
        Padding::Counted(4)
    } else {
        Padding::Counted(2)
    }
}

impl Info {
    pub fn key(&self) -> Key {
        Key(*match self {
            Info::UnicodeName(_) => b"luni",
            Info::NameSource(_) => b"lnsr",
            Info::Id(_) => b"lyid",
            Info::Version(_) => b"lyvr",
            Info::Section(section) if section.nested => b"lsdk",
            Info::Section(_) => b"lsct",
            Info::SheetColor(_) => b"lclr",
            Info::Knockout(_) => b"knko",
            Info::BlendClipped(_) => b"clbl",
            Info::BlendInterior(_) => b"infx",
            Info::FillOpacity(_) => b"iOpa",
            Info::TransparencyShapes(_) => b"tsly",
            Info::LayerMaskAsGlobal(_) => b"lmgm",
            Info::VectorMaskAsGlobal(_) => b"vmgm",
            Info::Protection(_) => b"lspf",
            Info::ReferencePoint { .. } => b"fxrp",
            Info::AlignedRendering(_) => b"sn2P",
            Info::ForeignEffectId(_) => b"ffxi",
            Info::VectorOriginationFlag(_) => b"vowv",
            Info::ChannelRestrictions(_) => b"brst",
            Info::Metadata(_) => b"shmd",
            Info::VectorMask(mask) if mask.stroke_variant => b"vsms",
            Info::VectorMask(_) => b"vmsk",
            Info::LegacyEffects(_) => b"lrFX",
            Info::TypeTool(_) => b"TySh",
            Info::PlacedLayer(placed) if placed.legacy_key => b"plLd",
            Info::PlacedLayer(_) => b"PlLd",
            Info::LinkedFiles(files) => return files.key.key(),
            Info::Patterns { key, .. } => return *key,
            Info::Annotations(_) => b"Anno",
            Info::FilterMask(_) => b"FMsk",
            Info::UserMask(_) => b"LMsk",
            Info::FilterEffects(effects) if effects.legacy_key => b"FXid",
            Info::FilterEffects(_) => b"FEid",
            Info::Adjustment(adjustment) => return adjustment.key(),
            Info::Descriptor(block) => return block.key,
            Info::Raw { key, .. } => return *key,
        })
    }

    pub fn descriptor(key: Key, descriptor: crate::descriptor::Descriptor) -> Self {
        Info::Descriptor(DescriptorBlock::new(key, descriptor))
    }

    fn decode(key: Key, reader: &mut Reader<'_>) -> Result<Self> {
        Ok(match &key.0 {
            b"luni" => Info::UnicodeName(reader.unicode()?),
            b"lnsr" => Info::NameSource(reader.key()?),
            b"lyid" => Info::Id(reader.u32()?),
            b"lyvr" => Info::Version(reader.u32()?),
            b"lsct" => Info::Section(Section::read(reader, false)?),
            b"lsdk" => Info::Section(Section::read(reader, true)?),
            b"lclr" => {
                let color = SheetColor::from_code(reader.u16()?);
                reader.skip(6)?;
                Info::SheetColor(color)
            }
            b"knko" => Info::Knockout(Knockout::from_code(reader.u8()?)),
            b"clbl" => Info::BlendClipped(reader.bool()?),
            b"infx" => Info::BlendInterior(reader.bool()?),
            b"iOpa" => Info::FillOpacity(reader.u8()?),
            b"tsly" => Info::TransparencyShapes(reader.bool()?),
            b"lmgm" => Info::LayerMaskAsGlobal(reader.bool()?),
            b"vmgm" => Info::VectorMaskAsGlobal(reader.bool()?),
            b"lspf" => Info::Protection(Protection(reader.u32()?)),
            b"fxrp" => Info::ReferencePoint {
                x: reader.f64()?,
                y: reader.f64()?,
            },
            b"sn2P" => Info::AlignedRendering(reader.u32()?),
            b"ffxi" => Info::ForeignEffectId(reader.u32()?),
            b"vowv" => Info::VectorOriginationFlag(reader.u32()?),
            b"brst" => {
                let mut channels = Vec::new();
                while reader.remaining() >= 4 {
                    channels.push(reader.i32()?);
                }
                Info::ChannelRestrictions(channels)
            }
            b"shmd" => Info::Metadata(simple::read_metadata(reader)?),
            b"vmsk" => Info::VectorMask(VectorMask::read(reader, false)?),
            b"vsms" => Info::VectorMask(VectorMask::read(reader, true)?),
            b"lrFX" => Info::LegacyEffects(LegacyEffects::read(reader)?),
            b"TySh" => Info::TypeTool(TypeTool::read(reader)?),
            b"PlLd" => Info::PlacedLayer(PlacedLayer::read(reader, false)?),
            b"plLd" => Info::PlacedLayer(PlacedLayer::read(reader, true)?),
            b"lnk2" | b"lnkD" | b"lnk3" | b"lnkE" => {
                let linked_key =
                    LinkedKey::from_key(key).ok_or_else(|| reader.malformed("linked files"))?;
                Info::LinkedFiles(LinkedFiles::read(reader, linked_key)?)
            }
            b"Patt" | b"Pat2" | b"Pat3" => Info::Patterns {
                key,
                patterns: Pattern::read_list(reader)?,
            },
            b"Anno" => Info::Annotations(Annotations::read(reader)?),
            b"FMsk" => Info::FilterMask(FilterMask::read(reader)?),
            b"LMsk" => Info::UserMask(UserMask::read(reader)?),
            b"FEid" => Info::FilterEffects(FilterEffects::read(reader, false)?),
            b"FXid" => Info::FilterEffects(FilterEffects::read(reader, true)?),
            _ if adjustment::is_adjustment_key(key) => {
                Info::Adjustment(Adjustment::read(key, reader)?)
            }
            _ if descriptor_block::is_descriptor_key(key) => {
                Info::Descriptor(DescriptorBlock::read(key, reader)?)
            }
            _ => Info::Raw {
                key,
                data: reader.take_rest().to_vec(),
            },
        })
    }

    fn encode(&self, writer: &mut Writer) -> Result<()> {
        match self {
            Info::UnicodeName(name) => writer.unicode(name),
            Info::NameSource(key) => writer.key(*key),
            Info::Id(value)
            | Info::Version(value)
            | Info::AlignedRendering(value)
            | Info::ForeignEffectId(value)
            | Info::VectorOriginationFlag(value) => writer.u32(*value),
            Info::Section(section) => section.write(writer),
            Info::SheetColor(color) => {
                writer.u16(color.code());
                writer.zeros(6);
            }
            Info::Knockout(knockout) => {
                writer.u8(knockout.code());
                writer.zeros(3);
            }
            Info::BlendClipped(value)
            | Info::BlendInterior(value)
            | Info::TransparencyShapes(value)
            | Info::LayerMaskAsGlobal(value)
            | Info::VectorMaskAsGlobal(value) => {
                writer.bool(*value);
                writer.zeros(3);
            }
            Info::FillOpacity(value) => {
                writer.u8(*value);
                writer.zeros(3);
            }
            Info::Protection(protection) => writer.u32(protection.0),
            Info::ReferencePoint { x, y } => {
                writer.f64(*x);
                writer.f64(*y);
            }
            Info::ChannelRestrictions(channels) => {
                channels.iter().for_each(|&channel| writer.i32(channel))
            }
            Info::Metadata(items) => simple::write_metadata(writer, items)?,
            Info::VectorMask(mask) => mask.write(writer),
            Info::LegacyEffects(effects) => effects.write(writer),
            Info::TypeTool(tool) => tool.write(writer)?,
            Info::PlacedLayer(placed) => placed.write(writer)?,
            Info::LinkedFiles(files) => files.write(writer)?,
            Info::Patterns { patterns, .. } => Pattern::write_list(writer, patterns)?,
            Info::Annotations(annotations) => annotations.write(writer)?,
            Info::FilterMask(mask) => mask.write(writer),
            Info::UserMask(mask) => mask.write(writer),
            Info::FilterEffects(effects) => effects.write(writer)?,
            Info::Adjustment(adjustment) => adjustment.write(writer),
            Info::Descriptor(block) => block.write(writer)?,
            Info::Raw { data, .. } => writer.bytes(data),
        }
        Ok(())
    }
}

pub(crate) struct RawBlock<'a> {
    pub(crate) key: Key,
    pub(crate) body: Reader<'a>,
}

pub(crate) fn read_raw_blocks<'a>(reader: &mut Reader<'a>, psb: bool) -> Result<Vec<RawBlock<'a>>> {
    let mut blocks = Vec::new();
    loop {
        reader.skip_zero_padding(3);
        if reader.remaining() < 12 {
            break;
        }
        let signature = reader.key()?;
        if !matches!(&signature.0, b"8BIM" | b"8B64") {
            return Err(reader.malformed("additional information signature"));
        }
        let key = reader.key()?;
        let wide = &signature.0 == b"8B64" || is_wide(key, psb);
        let mut probe = reader.clone();
        let mut length = probe.length(wide)?;
        if wide && length > probe.remaining() as u64 {
            let mut narrow = reader.clone();
            let short = u64::from(narrow.u32()?);
            if short <= narrow.remaining() as u64 {
                probe = narrow;
                length = short;
            }
        }
        *reader = probe;
        let body = reader.sub(length.min(reader.remaining() as u64))?;
        blocks.push(RawBlock { key, body });
    }
    Ok(blocks)
}

pub(crate) fn decode_block(block: RawBlock<'_>) -> Info {
    let RawBlock { key, mut body } = block;
    let raw = body.rest();
    match Info::decode(key, &mut body) {
        Ok(info) if body.finish_padded() => info,
        _ => Info::Raw {
            key,
            data: raw.to_vec(),
        },
    }
}

pub(crate) fn read_blocks(reader: &mut Reader<'_>, psb: bool) -> Result<Vec<Info>> {
    Ok(read_raw_blocks(reader, psb)?
        .into_iter()
        .map(decode_block)
        .collect())
}

pub(crate) fn write_block(writer: &mut Writer, info: &Info, psb: bool) -> Result<()> {
    write_keyed(writer, info.key(), psb, |writer| info.encode(writer))
}

pub(crate) fn write_keyed(
    writer: &mut Writer,
    key: Key,
    psb: bool,
    body: impl FnOnce(&mut Writer) -> Result<()>,
) -> Result<()> {
    let wide = is_wide(key, psb);
    writer.code(b"8BIM");
    writer.key(key);
    writer.section(Width::from_wide(wide), padding(key), body)
}
