use crate::starts::start_codes;

pub const START_CODE: [u8; 4] = [0, 0, 0, 1];

#[must_use]
pub fn units(stream: &[u8]) -> Vec<&[u8]> {
    let starts = start_codes(stream);
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, &(_, header))| {
            let end = starts.get(index + 1).map_or(stream.len(), |next| next.0);
            (end > header).then(|| &stream[header..end])
        })
        .collect()
}
