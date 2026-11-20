use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};

pub(crate) fn decoded(data: &str) -> Option<Vec<u8>> {
    let tight: String = data.split_whitespace().collect();
    STANDARD
        .decode(&tight)
        .or_else(|_| STANDARD_NO_PAD.decode(&tight))
        .ok()
}
