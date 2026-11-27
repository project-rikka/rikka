use base64::Engine;
use percent_encoding::percent_decode_str;

const BASE64: &str = "base64";

#[must_use]
pub fn data_url_payload(url: &str) -> Option<Vec<u8>> {
    let (header, payload) = url.strip_prefix("data:")?.split_once(',')?;
    let encoded = header
        .rsplit(';')
        .next()
        .is_some_and(|parameter| parameter.trim().eq_ignore_ascii_case(BASE64));
    match encoded {
        true => {
            let packed: Vec<u8> = percent_decode_str(payload)
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect();
            base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(trimmed(&packed))
                .ok()
        }
        false => Some(percent_decode_str(payload).collect()),
    }
}

fn trimmed(packed: &[u8]) -> &[u8] {
    let padding = packed
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    &packed[..packed.len() - padding]
}
