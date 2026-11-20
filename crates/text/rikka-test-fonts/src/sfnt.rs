use crate::writer::Writer;

const TRUETYPE_VERSION: [u8; 4] = [0, 1, 0, 0];
const HEADER_SIZE: usize = 12;
const RECORD_SIZE: usize = 16;
const TABLE_ALIGNMENT: usize = 4;
const HEAD_SIZE: usize = 54;
const HEAD_UNITS_PER_EM: usize = 18;
const MAXP_VERSION_0_5: u32 = 0x0000_5000;

pub fn sfnt<T: AsRef<[u8]>>(tables: &[(&[u8; 4], T)]) -> Vec<u8> {
    assemble(
        TRUETYPE_VERSION,
        tables.iter().map(|(tag, data)| (**tag, data.as_ref())),
    )
}

pub fn head(units_per_em: u16) -> Vec<u8> {
    let mut head = vec![0u8; HEAD_SIZE];
    head[..4].copy_from_slice(&TRUETYPE_VERSION);
    head[HEAD_UNITS_PER_EM..HEAD_UNITS_PER_EM + 2].copy_from_slice(&units_per_em.to_be_bytes());
    head
}

pub fn maxp(glyph_count: u16) -> Vec<u8> {
    Writer::new()
        .u32(MAXP_VERSION_0_5)
        .u16(glyph_count)
        .finish()
}

pub fn without_table(font: &[u8], removed: &[u8; 4]) -> Option<Vec<u8>> {
    let (version, mut tables) = tables_of(font)?;
    tables.retain(|(tag, _)| tag != removed);
    Some(assemble(version, tables))
}

pub fn with_table(font: &[u8], added: &[u8; 4], data: &[u8]) -> Option<Vec<u8>> {
    let (version, mut tables) = tables_of(font)?;
    tables.retain(|(tag, _)| tag != added);
    tables.push((*added, data));
    Some(assemble(version, tables))
}

type Tables<'f> = Vec<([u8; 4], &'f [u8])>;

fn tables_of(font: &[u8]) -> Option<([u8; 4], Tables<'_>)> {
    let version: [u8; 4] = font.get(..4)?.try_into().ok()?;
    let count = usize::from(u16::from_be_bytes(font.get(4..6)?.try_into().ok()?));
    let mut tables = Vec::with_capacity(count + 1);
    for index in 0..count {
        let record = font.get(HEADER_SIZE + index * RECORD_SIZE..)?;
        let tag: [u8; 4] = record.get(..4)?.try_into().ok()?;
        let offset = u32::from_be_bytes(record.get(8..12)?.try_into().ok()?) as usize;
        let length = u32::from_be_bytes(record.get(12..16)?.try_into().ok()?) as usize;
        tables.push((tag, font.get(offset..offset.checked_add(length)?)?));
    }
    Some((version, tables))
}

fn assemble<'t>(
    version: [u8; 4],
    tables: impl IntoIterator<Item = ([u8; 4], &'t [u8])>,
) -> Vec<u8> {
    let mut tables: Vec<([u8; 4], &[u8])> = tables.into_iter().collect();
    tables.sort_by_key(|(tag, _)| *tag);
    let mut font = Writer::new();
    font.raw(&version)
        .u16(tables.len() as u16)
        .u16(0)
        .u16(0)
        .u16(0);
    let mut offset = HEADER_SIZE + tables.len() * RECORD_SIZE;
    for (tag, data) in &tables {
        font.raw(tag)
            .u32(0)
            .u32(offset as u32)
            .u32(data.len() as u32);
        offset += data.len().next_multiple_of(TABLE_ALIGNMENT);
    }
    for (_, data) in &tables {
        font.raw(data).align(TABLE_ALIGNMENT);
    }
    font.finish()
}
