use crate::av1::Still;
use crate::error::{Error, Result};
use crate::format::Format;

const COLOR_ITEM: u16 = 1;
const ALPHA_ITEM: u16 = 2;
const ESSENTIAL: u8 = 0x80;
const MDAT_HEADER: usize = 8;
const ALPHA_URN: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0";
const PRIMARIES_BT709: u16 = 1;
const TRANSFER_SRGB: u16 = 13;
const MATRIX_BT601: u16 = 6;

#[derive(Debug)]
pub(crate) struct Plane {
    pub(crate) still: Still,
    pub(crate) channels: u8,
    pub(crate) depth: u8,
}

#[derive(Debug)]
pub(crate) struct Picture<'p> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) color: Plane,
    pub(crate) alpha: Option<Plane>,
    pub(crate) profile: Option<&'p [u8]>,
}

impl Picture<'_> {
    pub(crate) fn to_file(&self) -> Result<Vec<u8>> {
        let ftyp = boxed(
            b"ftyp",
            &[&b"avif"[..], &[0; 4], b"avif", b"mif1", b"miaf"].concat(),
        );
        let sized = self.meta(0, 0)?;
        let color_at = ftyp.len() + sized.len() + MDAT_HEADER;
        let alpha_at = color_at + self.color.still.data.len();
        let meta = self.meta(color_at, alpha_at)?;
        let mut payload = self.color.still.data.clone();
        if let Some(alpha) = &self.alpha {
            payload.extend_from_slice(&alpha.still.data);
        }
        let mdat = boxed(b"mdat", &payload);
        Ok([ftyp, meta, mdat].concat())
    }

    fn meta(&self, color_at: usize, alpha_at: usize) -> Result<Vec<u8>> {
        let mut located = vec![(COLOR_ITEM, color_at, self.color.still.data.len())];
        if let Some(alpha) = &self.alpha {
            located.push((ALPHA_ITEM, alpha_at, alpha.still.data.len()));
        }
        let hdlr = full(b"hdlr", 0, &[&[0; 4][..], b"pict", &[0; 12], &[0]].concat());
        let pitm = full(b"pitm", 0, &COLOR_ITEM.to_be_bytes());
        let mut children = vec![hdlr, pitm, location(&located)?, self.information()];
        if self.alpha.is_some() {
            let auxl = boxed(
                b"auxl",
                &[
                    ALPHA_ITEM.to_be_bytes(),
                    1_u16.to_be_bytes(),
                    COLOR_ITEM.to_be_bytes(),
                ]
                .concat(),
            );
            children.push(full(b"iref", 0, &auxl));
        }
        children.push(self.properties());
        Ok(full(b"meta", 0, &children.concat()))
    }

    fn information(&self) -> Vec<u8> {
        let entry = |item: u16| {
            full_with(
                b"infe",
                2,
                0,
                &[&item.to_be_bytes()[..], &[0, 0], b"av01", &[0]].concat(),
            )
        };
        let mut entries = vec![entry(COLOR_ITEM)];
        if self.alpha.is_some() {
            entries.push(entry(ALPHA_ITEM));
        }
        let count = (entries.len() as u16).to_be_bytes();
        full(b"iinf", 0, &[&count[..], &entries.concat()].concat())
    }

    fn properties(&self) -> Vec<u8> {
        let mut properties = vec![
            full(
                b"ispe",
                0,
                &[self.width.to_be_bytes(), self.height.to_be_bytes()].concat(),
            ),
            pixel_information(&self.color),
            boxed(b"av1C", &self.color.still.config),
            boxed(
                b"colr",
                &[
                    &b"nclx"[..],
                    &PRIMARIES_BT709.to_be_bytes(),
                    &TRANSFER_SRGB.to_be_bytes(),
                    &MATRIX_BT601.to_be_bytes(),
                    &[0x80],
                ]
                .concat(),
            ),
        ];
        let mut color = vec![1, 2, ESSENTIAL | 3, 4];
        if let Some(profile) = self.profile {
            properties.push(boxed(b"colr", &[&b"prof"[..], profile].concat()));
            color.push(properties.len() as u8);
        }
        let mut associations = vec![(COLOR_ITEM, color)];
        if let Some(alpha) = &self.alpha {
            properties.push(pixel_information(alpha));
            let pixi = properties.len() as u8;
            properties.push(boxed(b"av1C", &alpha.still.config));
            let config = properties.len() as u8;
            properties.push(full(b"auxC", 0, ALPHA_URN));
            let auxiliary = properties.len() as u8;
            associations.push((
                ALPHA_ITEM,
                vec![1, pixi, ESSENTIAL | config, ESSENTIAL | auxiliary],
            ));
        }
        let mut map = (associations.len() as u32).to_be_bytes().to_vec();
        for (item, indices) in associations {
            map.extend_from_slice(&item.to_be_bytes());
            map.push(indices.len() as u8);
            map.extend_from_slice(&indices);
        }
        let ipco = boxed(b"ipco", &properties.concat());
        let ipma = full(b"ipma", 0, &map);
        boxed(b"iprp", &[ipco, ipma].concat())
    }
}

fn location(located: &[(u16, usize, usize)]) -> Result<Vec<u8>> {
    let wide = |value: usize| {
        u32::try_from(value)
            .map(u32::to_be_bytes)
            .map_err(|_| Error::unsupported(Format::Avif, "files over 4 GiB"))
    };
    let mut body = vec![0x44, 0x00];
    body.extend_from_slice(&(located.len() as u16).to_be_bytes());
    for &(item, at, length) in located {
        body.extend_from_slice(&item.to_be_bytes());
        body.extend_from_slice(&0_u16.to_be_bytes());
        body.extend_from_slice(&1_u16.to_be_bytes());
        body.extend_from_slice(&wide(at)?);
        body.extend_from_slice(&wide(length)?);
    }
    Ok(full(b"iloc", 0, &body))
}

fn pixel_information(plane: &Plane) -> Vec<u8> {
    let mut body = vec![plane.channels];
    body.extend(std::iter::repeat_n(
        plane.depth,
        usize::from(plane.channels),
    ));
    full(b"pixi", 0, &body)
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let size = (8 + body.len()) as u32;
    [&size.to_be_bytes()[..], kind, body].concat()
}

fn full(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
    full_with(kind, version, 0, body)
}

fn full_with(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let header = (u32::from(version) << 24) | (flags & 0x00FF_FFFF);
    boxed(kind, &[&header.to_be_bytes()[..], body].concat())
}
