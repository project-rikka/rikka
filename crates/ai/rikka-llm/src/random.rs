use rand::RngCore;

pub fn uuid() -> String {
    let mut data = bytes::<16>();
    data[6] = (data[6] & 0x0f) | 0x40;
    data[8] = (data[8] & 0x3f) | 0x80;

    let text = hex(&data);
    format!(
        "{}-{}-{}-{}-{}",
        &text[..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..],
    )
}

pub fn bytes<const N: usize>() -> [u8; N] {
    let mut data = [0_u8; N];
    rand::rng().fill_bytes(&mut data);
    data
}

pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[char; 16] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)]);
        text.push(DIGITS[usize::from(byte & 0x0f)]);
    }
    text
}
