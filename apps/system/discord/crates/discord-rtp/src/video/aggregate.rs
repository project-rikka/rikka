pub(crate) const AGGREGATE_SIZE_LEN: usize = 2;

pub(crate) fn aggregated(mut rest: &[u8]) -> Vec<&[u8]> {
    let mut units = Vec::new();
    while rest.len() > AGGREGATE_SIZE_LEN {
        let size = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
        let Some(unit) = rest.get(AGGREGATE_SIZE_LEN..AGGREGATE_SIZE_LEN + size) else {
            return Vec::new();
        };
        units.push(unit);
        rest = &rest[AGGREGATE_SIZE_LEN + size..];
    }
    units
}
