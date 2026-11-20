use std::io::ErrorKind;
use std::path::Path;

const LARGEST: usize = 32 * 1024;

pub(crate) struct Read {
    pub(crate) text: String,
    pub(crate) note: Option<String>,
}

pub(crate) fn read(path: &Path) -> Result<Option<Read>, String> {
    let mut text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("it could not be read: {error}")),
    };

    if text.trim().is_empty() {
        return Ok(None);
    }

    let mut note = None;
    if text.len() > LARGEST {
        note = Some(format!(
            "it is {} bytes, so only the first {LARGEST} of it were taken",
            text.len()
        ));
        let mut at = LARGEST;
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        text.truncate(at);
    }

    Ok(Some(Read { text, note }))
}
