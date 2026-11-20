use std::borrow::Cow;

use rikka_encoding::{Encoding, UTF_8, WINDOWS_1252};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Repair {
    Keep,
    Decode(&'static Encoding),
}

impl Repair {
    pub(crate) fn guess<'a>(texts: impl Iterator<Item = &'a str>) -> Repair {
        let suspects: Vec<Vec<u8>> = texts.filter_map(narrow_bytes).collect();
        if suspects.is_empty() {
            return Repair::Keep;
        }
        if suspects
            .iter()
            .all(|bytes| std::str::from_utf8(bytes).is_ok())
        {
            return Repair::Decode(UTF_8);
        }
        let encoding = rikka_encoding::guess(suspects.iter().map(Vec::as_slice), None);
        let reveals_script = encoding != WINDOWS_1252
            && suspects.iter().any(|bytes| {
                encoding
                    .decode_without_bom_handling_and_without_replacement(bytes)
                    .is_some_and(|decoded| decoded.chars().any(beyond_latin))
            });
        if reveals_script {
            Repair::Decode(encoding)
        } else {
            Repair::Keep
        }
    }

    pub(crate) fn apply<'a>(self, text: &'a str) -> Cow<'a, str> {
        let Repair::Decode(encoding) = self else {
            return Cow::Borrowed(text);
        };
        let Some(bytes) = narrow_bytes(text) else {
            return Cow::Borrowed(text);
        };
        encoding
            .decode_without_bom_handling_and_without_replacement(&bytes)
            .map_or(Cow::Borrowed(text), |decoded| {
                Cow::Owned(decoded.into_owned())
            })
    }
}

fn beyond_latin(character: char) -> bool {
    let code = u32::from(character);
    code >= 0x0370 && !(0x1E00..=0x1EFF).contains(&code) && !(0x2000..=0x206F).contains(&code)
}

fn narrow_bytes(text: &str) -> Option<Vec<u8>> {
    let mut wide = false;
    let bytes = text
        .chars()
        .map(|character| {
            let code = u32::from(character);
            wide |= code >= 0x80;
            u8::try_from(code).ok()
        })
        .collect::<Option<Vec<u8>>>()?;
    wide.then_some(bytes)
}
