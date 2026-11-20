use rikka_model_core::{ByteReader, Error, ImportCtx, Result, Stage};

use crate::facet::{Document, Facet, Palette, Solid, VALID};

pub(crate) const HEADER: usize = 80;
pub(crate) const PREAMBLE: usize = HEADER + 4;
pub(crate) const FACET: usize = 50;

const COLOR: &[u8] = b"COLOR=";
const PLAUSIBLE_MAGNITUDE: f32 = 1.0e9;

fn declared_count(bytes: &[u8]) -> Option<u32> {
    let mut reader = ByteReader::new(bytes);
    reader.seek(HEADER).ok()?;
    reader.u32().ok()
}

pub(crate) fn declared_length(bytes: &[u8]) -> Option<usize> {
    (declared_count(bytes)? as usize)
        .checked_mul(FACET)?
        .checked_add(PREAMBLE)
}

pub(crate) fn plausible(head: &[u8]) -> bool {
    let Some(count) = declared_count(head) else {
        return false;
    };
    if count == 0 {
        return head.len() == PREAMBLE;
    }
    let shown = head[PREAMBLE..]
        .as_chunks::<FACET>()
        .0
        .iter()
        .take(count as usize)
        .filter_map(|record| facet(record).ok())
        .collect::<Vec<_>>();
    !shown.is_empty()
        && shown.iter().all(|(facet, _)| {
            facet
                .corners
                .iter()
                .chain([&facet.normal])
                .all(|value| value.is_finite() && value.abs().max_element() < PLAUSIBLE_MAGNITUDE)
        })
}

pub(crate) fn generator(bytes: &[u8]) -> Option<String> {
    let header = bytes.get(..HEADER)?;
    let text = &header[..header.iter().position(|&byte| byte == 0).unwrap_or(HEADER)];
    let text = text
        .iter()
        .position(|&byte| !(byte.is_ascii_graphic() || byte == b' '))
        .map_or(text, |end| &text[..end]);
    let text = std::str::from_utf8(text).ok()?.trim();
    (!text.is_empty() && !text.to_ascii_lowercase().starts_with("solid")).then(|| text.to_owned())
}

pub(crate) fn parse(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<Document> {
    let length = declared_length(bytes).ok_or_else(|| {
        Error::Damaged(format!(
            "binary STL is shorter than {PREAMBLE} bytes: {} bytes",
            bytes.len()
        ))
    })?;
    if length > bytes.len() {
        return Err(Error::Damaged(format!(
            "header declares {} triangles but the file has {} bytes",
            (length - PREAMBLE) / FACET,
            bytes.len()
        )));
    }
    let header = &bytes[..HEADER];
    let object_color = object_color(header);
    let records = &bytes[PREAMBLE..length];
    let count = records.len() / FACET;

    let mut facets = Vec::with_capacity(count);
    let mut attributes = Vec::with_capacity(count);
    for (index, record) in records.as_chunks::<FACET>().0.iter().enumerate() {
        if index % 65_536 == 0 {
            ctx.checkpoint()?;
            ctx.report(Stage::Parsing, 0.7 * index as f32 / count.max(1) as f32);
        }
        let (facet, attribute) = facet(record)?;
        if facet.corners.iter().any(|corner| !corner.is_finite()) {
            return Err(Error::Damaged(format!(
                "triangle {index} has a non-finite vertex"
            )));
        }
        facets.push(facet);
        attributes.push(attribute);
    }

    let palette = match object_color {
        Some(_) => Some(Palette::Magics),
        None => attributes
            .iter()
            .any(|attribute| attribute & VALID != 0)
            .then_some(Palette::VisCam),
    };
    if let Some(palette) = palette {
        for (facet, &attribute) in facets.iter_mut().zip(&attributes) {
            facet.color = palette.decode(attribute);
        }
    }

    Ok(Document {
        solids: vec![Solid {
            name: String::new(),
            facets,
        }],
        object_color,
    })
}

fn facet(record: &[u8; FACET]) -> Result<(Facet, u16)> {
    let mut reader = ByteReader::new(record);
    let facet = Facet {
        normal: reader.vec3()?,
        corners: [reader.vec3()?, reader.vec3()?, reader.vec3()?],
        color: None,
    };
    Ok((facet, reader.u16()?))
}

fn object_color(header: &[u8]) -> Option<[u8; 4]> {
    let at = header
        .windows(COLOR.len())
        .position(|window| window == COLOR)?;
    let color = header.get(at + COLOR.len()..at + COLOR.len() + 4)?;
    Some(color.try_into().expect("four bytes"))
}
