use drm_fourcc::{DrmFourcc, DrmModifier};
use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};

pub(crate) fn drm_fourcc(fourcc: Fourcc) -> Option<DrmFourcc> {
    DrmFourcc::try_from(fourcc.0).ok()
}

pub(crate) fn drm_modifier(modifier: Modifier) -> DrmModifier {
    DrmModifier::from(modifier.0)
}

const BLOB_HEADER: usize = 24;
const MODIFIER_ENTRY: usize = 24;
const FORMATS_PER_ENTRY: u32 = 64;

pub(crate) fn parse_in_formats(blob: &[u8]) -> Option<FormatSet> {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_ne_bytes(blob.get(at..at + 4)?.try_into().ok()?))
    };
    let long = |at: usize| -> Option<u64> {
        Some(u64::from_ne_bytes(blob.get(at..at + 8)?.try_into().ok()?))
    };
    if blob.len() < BLOB_HEADER {
        return None;
    }
    let count_formats = word(8)? as usize;
    let formats_offset = word(12)? as usize;
    let count_modifiers = word(16)? as usize;
    let modifiers_offset = word(20)? as usize;

    let fourccs: Vec<u32> = (0..count_formats)
        .map(|index| word(formats_offset + index * 4))
        .collect::<Option<_>>()?;

    let mut formats = Vec::new();
    for index in 0..count_modifiers {
        let entry = modifiers_offset + index * MODIFIER_ENTRY;
        let mask = long(entry)?;
        let offset = word(entry + 8)?;
        let modifier = Modifier(long(entry + 16)?);
        for bit in 0..FORMATS_PER_ENTRY {
            if mask & (1 << bit) == 0 {
                continue;
            }
            let Some(fourcc) = fourccs.get((offset + bit) as usize) else {
                continue;
            };
            formats.push(Format::new(Fourcc(*fourcc), modifier));
        }
    }
    Some(formats.into_iter().collect())
}
