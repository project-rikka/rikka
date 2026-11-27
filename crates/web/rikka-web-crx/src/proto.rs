const LENGTH_DELIMITED: u64 = 2;

fn varint(value: u64, into: &mut Vec<u8>) {
    let mut rest = value;
    while rest >= 0x80 {
        into.push((rest as u8 & 0x7f) | 0x80);
        rest >>= 7;
    }
    into.push(rest as u8);
}

pub(crate) fn bytes_field(field: u64, value: &[u8], into: &mut Vec<u8>) {
    varint(field << 3 | LENGTH_DELIMITED, into);
    varint(value.len() as u64, into);
    into.extend_from_slice(value);
}
