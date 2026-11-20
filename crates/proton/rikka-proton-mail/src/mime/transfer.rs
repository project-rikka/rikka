use std::fmt;

use rikka_proton_contentline::decode_quoted_printable;

const LINE: usize = 76;

pub(crate) fn decode(body: &[u8], encoding: Option<&str>) -> Vec<u8> {
    match encoding.map(|name| name.trim().to_ascii_lowercase()) {
        Some(name) if name == "base64" => base64(body),
        Some(name) if name == "quoted-printable" => decode_quoted_printable(body),
        _ => body.to_vec(),
    }
}

pub(crate) fn base64(input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(input.len() / 4 * 3);
    let mut collected = 0u32;
    let mut accumulator = 0u8;

    for byte in input {
        let Some(sextet) = sextet(*byte) else {
            if *byte == b'=' {
                break;
            }
            continue;
        };

        collected = (collected << 6) | u32::from(sextet);
        accumulator += 1;

        if accumulator == 4 {
            decoded.extend_from_slice(&collected.to_be_bytes()[1..]);
            collected = 0;
            accumulator = 0;
        }
    }

    match accumulator {
        2 => decoded.push((collected >> 4) as u8),
        3 => {
            decoded.push((collected >> 10) as u8);
            decoded.push((collected >> 2) as u8);
        }
        _ => {}
    }

    decoded
}

pub(crate) fn base64_lines(data: &[u8]) -> Vec<u8> {
    let encoded = rikka_proton_api::base64::encode(data);
    let mut wrapped = Vec::with_capacity(encoded.len() + encoded.len() / LINE * 2 + 2);

    for line in encoded.as_bytes().chunks(LINE) {
        wrapped.extend_from_slice(line);
        wrapped.extend_from_slice(b"\r\n");
    }

    wrapped
}

pub(crate) fn write_quoted_printable(out: &mut impl fmt::Write, text: &str) -> fmt::Result {
    let normalized = text.replace("\r\n", "\n");
    for (index, line) in normalized.split('\n').enumerate() {
        if index > 0 {
            out.write_str("\r\n")?;
        }
        let bytes = line.as_bytes();
        let mut width = 0;
        for (position, byte) in bytes.iter().enumerate() {
            let is_last = position + 1 == bytes.len();
            let is_literal = match byte {
                b' ' | b'\t' => !is_last,
                b'=' => false,
                33..=126 => true,
                _ => false,
            };
            let encoded_len = if is_literal { 1 } else { 3 };
            if width + encoded_len > LINE - 1 {
                out.write_str("=\r\n")?;
                width = 0;
            }
            width += encoded_len;
            match is_literal {
                true => out.write_char(char::from(*byte))?,
                false => write!(out, "={byte:02X}")?,
            }
        }
    }
    Ok(())
}

fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}
