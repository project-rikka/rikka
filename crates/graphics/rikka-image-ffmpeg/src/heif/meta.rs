use std::borrow::Cow;

use crate::atoms::{Atom, Atoms, top_level};
use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;

const MOST_META_BYTES: u64 = 64 << 20;
const COLOUR_PROFILE: [u8; 4] = *b"prof";
const RESTRICTED_PROFILE: [u8; 4] = *b"rICC";
const COLOUR_CODES: [u8; 4] = *b"nclx";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Nclx {
    pub(crate) primaries: u16,
    pub(crate) transfer: u16,
    pub(crate) matrix: u16,
    pub(crate) full_range: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Aperture {
    pub(crate) width: (u32, u32),
    pub(crate) height: (u32, u32),
    pub(crate) horizontal: (i32, u32),
    pub(crate) vertical: (i32, u32),
}

#[derive(Clone, Debug)]
pub(crate) enum Property {
    Spatial { width: u32, height: u32 },
    Av1Config(Vec<u8>),
    HevcConfig(Vec<u8>),
    Codes(Nclx),
    Profile(Vec<u8>),
    Rotation(u8),
    Mirror(u8),
    Auxiliary(Vec<u8>),
    Aperture(Aperture),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
    File,
    Idat,
}

#[derive(Clone, Debug)]
struct Location {
    item: u32,
    base: Base,
    extents: Vec<(u64, u64)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Reference {
    pub(crate) kind: [u8; 4],
    pub(crate) from: u32,
    pub(crate) to: Vec<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct Meta {
    pub(crate) format: Format,
    pub(crate) primary: u32,
    items: Vec<(u32, [u8; 4])>,
    locations: Vec<Location>,
    pub(crate) references: Vec<Reference>,
    properties: Vec<Property>,
    associations: Vec<(u32, Vec<usize>)>,
    idat: Vec<u8>,
}

impl Meta {
    pub(crate) fn read(fetch: &mut Fetch<'_>) -> Result<Meta> {
        let format = fetch.format();
        let span = top_level(fetch)?
            .into_iter()
            .find(|span| span.kind == *b"meta")
            .ok_or_else(|| Error::malformed(format, "file without a meta box"))?;
        if span.len > MOST_META_BYTES {
            return Err(Error::unsupported(format, "metadata larger than 64 MiB"));
        }
        let body = fetch.fetch(span.at, span.len)?;
        Meta::parse(format, &body)
    }

    fn parse(format: Format, body: &[u8]) -> Result<Meta> {
        let (_, _, rest) = Atom {
            kind: *b"meta",
            body,
        }
        .full(format)?;
        let mut meta = Meta {
            format,
            primary: 0,
            items: Vec::new(),
            locations: Vec::new(),
            references: Vec::new(),
            properties: Vec::new(),
            associations: Vec::new(),
            idat: Vec::new(),
        };
        let mut primary = None;
        for atom in Atoms::new(format, rest.rest()) {
            let atom = atom?;
            match &atom.kind {
                b"pitm" => primary = Some(primary_item(format, atom)?),
                b"iloc" => meta.locations = locations(format, atom)?,
                b"iinf" => meta.items = items(format, atom)?,
                b"iref" => meta.references = references(format, atom)?,
                b"iprp" => (meta.properties, meta.associations) = item_properties(format, atom)?,
                b"idat" => meta.idat = atom.body.to_vec(),
                _ => {}
            }
        }
        meta.primary =
            primary.ok_or_else(|| Error::malformed(format, "file without a primary item"))?;
        Ok(meta)
    }

    pub(crate) fn kind(&self, item: u32) -> Option<[u8; 4]> {
        self.items
            .iter()
            .find(|(id, _)| *id == item)
            .map(|(_, kind)| *kind)
    }

    pub(crate) fn properties(&self, item: u32) -> impl Iterator<Item = &Property> {
        self.associations
            .iter()
            .filter(move |(id, _)| *id == item)
            .flat_map(|(_, indexes)| indexes.iter())
            .filter_map(|&index| self.properties.get(index))
    }

    pub(crate) fn referenced(&self, kind: [u8; 4], from: u32) -> Option<&[u32]> {
        self.references
            .iter()
            .find(|reference| reference.kind == kind && reference.from == from)
            .map(|reference| reference.to.as_slice())
    }

    pub(crate) fn referring(&self, kind: [u8; 4], to: u32) -> impl Iterator<Item = u32> + '_ {
        self.references
            .iter()
            .filter(move |reference| reference.kind == kind && reference.to.contains(&to))
            .map(|reference| reference.from)
    }

    pub(crate) fn item_data<'d>(&'d self, item: u32, data: &'d [u8]) -> Result<Cow<'d, [u8]>> {
        let location = self.location(item)?;
        let mut pieces = Vec::with_capacity(location.extents.len());
        for &(offset, length) in &location.extents {
            pieces.push(self.extent(location.base, offset, length, data)?);
        }
        Ok(match pieces.as_slice() {
            [single] => Cow::Borrowed(*single),
            many => Cow::Owned(many.concat()),
        })
    }

    pub(crate) fn item_head(&self, item: u32, fetch: &mut Fetch<'_>, most: u64) -> Result<Vec<u8>> {
        let location = self.location(item)?;
        let &(offset, length) = location
            .extents
            .first()
            .ok_or_else(|| Error::malformed(self.format, "item without data"))?;
        let head = match length {
            0 => most,
            length => length.min(most),
        };
        match location.base {
            Base::Idat => {
                let whole = self.extent(Base::Idat, offset, length, &[])?;
                Ok(whole[..whole.len().min(head as usize)].to_vec())
            }
            Base::File => Ok(fetch.fetch_up_to(offset, head)?.into_owned()),
        }
    }

    fn location(&self, item: u32) -> Result<&Location> {
        self.locations
            .iter()
            .find(|location| location.item == item)
            .ok_or_else(|| Error::malformed(self.format, "item without a location"))
    }

    fn extent<'d>(
        &'d self,
        base: Base,
        offset: u64,
        length: u64,
        data: &'d [u8],
    ) -> Result<&'d [u8]> {
        let source = match base {
            Base::File => data,
            Base::Idat => &self.idat,
        };
        let start = usize::try_from(offset).map_err(|_| Error::Truncated(self.format))?;
        let end = match length {
            0 => Some(source.len()),
            length => usize::try_from(length)
                .ok()
                .and_then(|length| start.checked_add(length)),
        };
        end.and_then(|end| source.get(start..end))
            .ok_or(Error::Truncated(self.format))
    }
}

fn primary_item(format: Format, atom: Atom<'_>) -> Result<u32> {
    let (version, _, mut bytes) = atom.full(format)?;
    match version {
        0 => bytes.u16_be().map(u32::from),
        _ => bytes.u32_be(),
    }
}

fn locations(format: Format, atom: Atom<'_>) -> Result<Vec<Location>> {
    let (version, _, mut bytes) = atom.full(format)?;
    let sizes = bytes.u16_be()?;
    let offset_size = (sizes >> 12) as u8;
    let length_size = ((sizes >> 8) & 0xF) as u8;
    let base_offset_size = ((sizes >> 4) & 0xF) as u8;
    let index_size = match version {
        1 | 2 => (sizes & 0xF) as u8,
        _ => 0,
    };
    let count = match version {
        0 | 1 => u32::from(bytes.u16_be()?),
        2 => bytes.u32_be()?,
        _ => return Err(Error::unsupported(format, "item location version")),
    };
    let mut locations = Vec::new();
    for _ in 0..count {
        let item = match version {
            2 => bytes.u32_be()?,
            _ => u32::from(bytes.u16_be()?),
        };
        let method = match version {
            1 | 2 => bytes.u16_be()? & 0xF,
            _ => 0,
        };
        let reference = bytes.u16_be()?;
        let base_offset = bytes.sized_be(base_offset_size)?;
        let extent_count = bytes.u16_be()?;
        let mut extents = Vec::new();
        for _ in 0..extent_count {
            bytes.sized_be(index_size)?;
            let offset = bytes.sized_be(offset_size)?;
            let length = bytes.sized_be(length_size)?;
            let offset = base_offset
                .checked_add(offset)
                .ok_or_else(|| Error::malformed(format, "item location"))?;
            extents.push((offset, length));
        }
        let base = match (method, reference) {
            (0, 0) => Base::File,
            (1, _) => Base::Idat,
            (0, _) => return Err(Error::unsupported(format, "items stored in other files")),
            _ => return Err(Error::unsupported(format, "items built from other items")),
        };
        locations.push(Location {
            item,
            base,
            extents,
        });
    }
    Ok(locations)
}

fn items(format: Format, atom: Atom<'_>) -> Result<Vec<(u32, [u8; 4])>> {
    let (version, _, mut bytes) = atom.full(format)?;
    match version {
        0 => bytes.skip(2)?,
        _ => bytes.skip(4)?,
    }
    let mut items = Vec::new();
    for entry in Atoms::new(format, bytes.rest()) {
        let entry = entry?;
        if entry.kind != *b"infe" {
            continue;
        }
        let (version, _, mut entry) = entry.full(format)?;
        if version < 2 {
            continue;
        }
        let id = match version {
            2 => u32::from(entry.u16_be()?),
            _ => entry.u32_be()?,
        };
        entry.skip(2)?;
        items.push((id, entry.fourcc()?));
    }
    Ok(items)
}

fn references(format: Format, atom: Atom<'_>) -> Result<Vec<Reference>> {
    let (version, _, bytes) = atom.full(format)?;
    let mut references = Vec::new();
    for reference in Atoms::new(format, bytes.rest()) {
        let reference = reference?;
        let mut bytes = Bytes::new(format, reference.body);
        let id = |bytes: &mut Bytes<'_>| match version {
            0 => bytes.u16_be().map(u32::from),
            _ => bytes.u32_be(),
        };
        let from = id(&mut bytes)?;
        let count = bytes.u16_be()?;
        let mut to = Vec::with_capacity(usize::from(count).min(bytes.remaining() / 2));
        for _ in 0..count {
            to.push(id(&mut bytes)?);
        }
        references.push(Reference {
            kind: reference.kind,
            from,
            to,
        });
    }
    Ok(references)
}

type Associations = Vec<(u32, Vec<usize>)>;

fn item_properties(format: Format, atom: Atom<'_>) -> Result<(Vec<Property>, Associations)> {
    let container = Atoms::find(format, atom.body, *b"ipco")?
        .ok_or_else(|| Error::malformed(format, "item properties without a container"))?;
    let mut properties = Vec::new();
    for property in Atoms::new(format, container.body) {
        properties.push(property_of(format, property?)?);
    }
    let mut associations = Vec::new();
    for association in Atoms::new(format, atom.body) {
        let association = association?;
        if association.kind != *b"ipma" {
            continue;
        }
        let (version, flags, mut bytes) = association.full(format)?;
        let count = bytes.u32_be()?;
        for _ in 0..count {
            let item = match version {
                0 => u32::from(bytes.u16_be()?),
                _ => bytes.u32_be()?,
            };
            let entries = bytes.u8()?;
            let mut indexes = Vec::with_capacity(usize::from(entries));
            for _ in 0..entries {
                let index = match flags & 1 {
                    1 => usize::from(bytes.u16_be()? & 0x7FFF),
                    _ => usize::from(bytes.u8()? & 0x7F),
                };
                if index > 0 {
                    indexes.push(index - 1);
                }
            }
            associations.push((item, indexes));
        }
    }
    Ok((properties, associations))
}

fn property_of(format: Format, atom: Atom<'_>) -> Result<Property> {
    let mut bytes = Bytes::new(format, atom.body);
    Ok(match &atom.kind {
        b"ispe" => {
            bytes.skip(4)?;
            Property::Spatial {
                width: bytes.u32_be()?,
                height: bytes.u32_be()?,
            }
        }
        b"av1C" => Property::Av1Config(atom.body.to_vec()),
        b"hvcC" => Property::HevcConfig(atom.body.to_vec()),
        b"colr" => match bytes.fourcc()? {
            COLOUR_CODES => Property::Codes(Nclx {
                primaries: bytes.u16_be()?,
                transfer: bytes.u16_be()?,
                matrix: bytes.u16_be()?,
                full_range: bytes.u8()? & 0x80 != 0,
            }),
            COLOUR_PROFILE | RESTRICTED_PROFILE => Property::Profile(bytes.rest().to_vec()),
            _ => Property::Other,
        },
        b"irot" => Property::Rotation(bytes.u8()? & 0x3),
        b"imir" => Property::Mirror(bytes.u8()? & 0x1),
        b"auxC" => {
            bytes.skip(4)?;
            Property::Auxiliary(bytes.text()?.to_vec())
        }
        b"clap" => Property::Aperture(Aperture {
            width: (bytes.u32_be()?, bytes.u32_be()?),
            height: (bytes.u32_be()?, bytes.u32_be()?),
            horizontal: (bytes.i32_be()?, bytes.u32_be()?),
            vertical: (bytes.i32_be()?, bytes.u32_be()?),
        }),
        _ => Property::Other,
    })
}
