use std::path::PathBuf;

use rikka_file_format::{BinaryFormat, DecodeError, Reader, Writer};
use rikka_text_font::Tag;

use crate::coverage::{PAGE_COUNT, Page};
use crate::discovery::FileStamp;
use crate::face::{Axis, AxisRange, ColorFormats, FaceRecord, Slope};
use crate::index::{checksum, path};

const FORMAT: BinaryFormat = BinaryFormat::new("font index", *b"RKFONTIX");
const PAGE_BYTES: usize = 32;
const FILE_BYTES: usize = 40;
const FACE_BYTES: usize = 50;
const AXIS_BYTES: usize = 17;
const COVERAGE_BYTES: usize = 6;

const UPRIGHT: u8 = 1;
const ITALIC: u8 = 2;
const OBLIQUE: u8 = 4;
const COLR: u8 = 1;
const CBDT: u8 = 2;
const SBIX: u8 = 4;
const SVG: u8 = 8;

#[derive(Debug, Default)]
pub(crate) struct IndexContents {
    pub(crate) pages: Vec<Page>,
    pub(crate) files: Vec<IndexedFile>,
}

#[derive(Debug)]
pub(crate) struct IndexedFile {
    pub(crate) path: PathBuf,
    pub(crate) stamp: FileStamp,
    pub(crate) faces: Vec<FaceRecord>,
}

pub(crate) fn encode(contents: &IndexContents) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut writer = Writer::new(&mut bytes);
    FORMAT.write_header(&mut writer);
    writer.count32(contents.pages.len());
    for page in &contents.pages {
        writer.put_all(page);
    }
    let files: Vec<(&[u8], &IndexedFile)> = contents
        .files
        .iter()
        .filter_map(|file| Some((path::encode(&file.path)?, file)))
        .collect();
    writer.count32(files.len());
    for (path, file) in files {
        writer.bytes(path);
        let stamp = &file.stamp;
        writer.put(&stamp.size);
        writer.put(&stamp.modified);
        writer.put(&stamp.modified_nanos);
        writer.put(&stamp.inode);
        writer.put(&stamp.device);
        writer.count32(file.faces.len());
        for face in &file.faces {
            encode_face(&mut writer, face);
        }
    }
    checksum::seal(bytes)
}

fn encode_face(writer: &mut Writer<'_>, face: &FaceRecord) {
    writer.put(&face.index);
    writer.count32(face.families.len());
    for family in &face.families {
        writer.str(family);
    }
    writer.str(&face.style_name);
    writer.str(&face.full_name);
    for range in [face.weight, face.width] {
        writer.put_all(&[range.min, range.default, range.max]);
    }
    let slope = face.slope;
    let (low, high) = slope.oblique.unwrap_or_default();
    writer.tag(pack(&[
        (slope.upright, UPRIGHT),
        (slope.italic, ITALIC),
        (slope.oblique.is_some(), OBLIQUE),
    ]));
    writer.put_all(&[low, high]);
    writer.put(&face.monospace);
    let color = face.color;
    writer.tag(pack(&[
        (color.colr, COLR),
        (color.cbdt, CBDT),
        (color.sbix, SBIX),
        (color.svg, SVG),
    ]));
    writer.count32(face.axes.len());
    for axis in &face.axes {
        writer.put(&axis.tag.to_u32());
        writer.put_all(&[axis.min, axis.default, axis.max]);
        writer.put(&axis.hidden);
    }
    writer.count32(face.scripts.len());
    for script in &face.scripts {
        writer.put(&script.to_u32());
    }
    writer.count32(face.coverage.len());
    for &(page, slot) in &face.coverage {
        writer.put(&page);
        writer.put(&slot);
    }
}

fn pack(flags: &[(bool, u8)]) -> u8 {
    flags
        .iter()
        .filter(|(set, _)| *set)
        .fold(0, |packed, (_, bit)| packed | bit)
}

pub(crate) fn decode(bytes: &[u8]) -> Option<IndexContents> {
    let body = checksum::open(bytes)?;
    decode_contents(&mut Reader::new(body)).ok()
}

fn decode_contents(reader: &mut Reader<'_>) -> Result<IndexContents, DecodeError> {
    FORMAT.read_header(reader)?;
    let page_count = reader.count32(PAGE_BYTES)?;
    let pages = (0..page_count)
        .map(|_| {
            let bytes = reader.array::<PAGE_BYTES>()?;
            let (words, _) = bytes.as_chunks::<8>();
            Ok(std::array::from_fn(|index| {
                u64::from_le_bytes(words[index])
            }))
        })
        .collect::<Result<Vec<Page>, DecodeError>>()?;
    let file_count = reader.count32(FILE_BYTES)?;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let path = path::decode(reader.bytes()?)?;
        let stamp = FileStamp {
            size: reader.get()?,
            modified: reader.get()?,
            modified_nanos: reader.get()?,
            inode: reader.get()?,
            device: reader.get()?,
        };
        let face_count = reader.count32(FACE_BYTES)?;
        let faces = (0..face_count)
            .map(|_| decode_face(reader, pages.len()))
            .collect::<Result<Vec<_>, _>>()?;
        files.push(IndexedFile { path, stamp, faces });
    }
    reader.finish()?;
    Ok(IndexContents { pages, files })
}

fn decode_face(reader: &mut Reader<'_>, page_count: usize) -> Result<FaceRecord, DecodeError> {
    let index = reader.get()?;
    let family_count = reader.count32(4)?;
    if family_count == 0 {
        return Err(DecodeError::Malformed("face has no family"));
    }
    let families = (0..family_count)
        .map(|_| reader.str().map(Box::from))
        .collect::<Result<Vec<Box<str>>, _>>()?;
    let style_name = reader.str()?.into();
    let full_name = reader.str()?.into();
    let weight = range(reader)?;
    let width = range(reader)?;
    let slope_flags = reader.tag()?;
    let oblique = (finite(reader)?, finite(reader)?);
    let slope = Slope {
        upright: slope_flags & UPRIGHT != 0,
        italic: slope_flags & ITALIC != 0,
        oblique: (slope_flags & OBLIQUE != 0).then_some(oblique),
    };
    let monospace = reader.get()?;
    let color_flags = reader.tag()?;
    let color = ColorFormats {
        colr: color_flags & COLR != 0,
        cbdt: color_flags & CBDT != 0,
        sbix: color_flags & SBIX != 0,
        svg: color_flags & SVG != 0,
    };
    let axis_count = reader.count32(AXIS_BYTES)?;
    let axes = (0..axis_count)
        .map(|_| {
            Ok(Axis {
                tag: Tag::from_u32(reader.get()?),
                min: finite(reader)?,
                default: finite(reader)?,
                max: finite(reader)?,
                hidden: reader.get()?,
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    let script_count = reader.count32(4)?;
    let scripts = (0..script_count)
        .map(|_| reader.get().map(Tag::from_u32))
        .collect::<Result<Vec<_>, _>>()?;
    let coverage_count = reader.count32(COVERAGE_BYTES)?;
    let mut coverage = Vec::with_capacity(coverage_count);
    for _ in 0..coverage_count {
        let page: u16 = reader.get()?;
        let slot: u32 = reader.get()?;
        let ascending = coverage.last().is_none_or(|&(previous, _)| previous < page);
        if usize::from(page) >= PAGE_COUNT || slot as usize >= page_count || !ascending {
            return Err(DecodeError::Malformed(
                "coverage entry out of order or range",
            ));
        }
        coverage.push((page, slot));
    }
    Ok(FaceRecord {
        index,
        families,
        style_name,
        full_name,
        weight,
        width,
        slope,
        monospace,
        color,
        axes,
        scripts,
        coverage,
    })
}

fn range(reader: &mut Reader<'_>) -> Result<AxisRange, DecodeError> {
    Ok(AxisRange {
        min: finite(reader)?,
        default: finite(reader)?,
        max: finite(reader)?,
    })
}

fn finite(reader: &mut Reader<'_>) -> Result<f32, DecodeError> {
    let value: f32 = reader.get()?;
    match value.is_finite() {
        true => Ok(value),
        false => Err(DecodeError::Malformed("value is not finite")),
    }
}
