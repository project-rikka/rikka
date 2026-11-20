use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Locale {
    language: [u8; 3],
    script: [u8; 4],
    region: [u8; 3],
}

impl Locale {
    pub fn parse(tag: &str) -> Option<Self> {
        let tag = tag.split(['.', '@']).next().unwrap_or_default();
        let mut subtags = tag.split(['-', '_']);
        let language = subtags.next()?;
        if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic())
        {
            return None;
        }
        let mut locale = Self {
            language: [0; 3],
            script: [0; 4],
            region: [0; 3],
        };
        copy_mapped(&mut locale.language, language, u8::to_ascii_lowercase);
        for subtag in subtags {
            let alphabetic = subtag.bytes().all(|b| b.is_ascii_alphabetic());
            let numeric = subtag.bytes().all(|b| b.is_ascii_digit());
            match subtag.len() {
                4 if alphabetic && locale.script[0] == 0 && locale.region[0] == 0 => {
                    copy_mapped(&mut locale.script, subtag, u8::to_ascii_lowercase);
                    locale.script[0] = locale.script[0].to_ascii_uppercase();
                }
                2 if alphabetic && locale.region[0] == 0 => {
                    copy_mapped(&mut locale.region, subtag, u8::to_ascii_uppercase);
                }
                3 if numeric && locale.region[0] == 0 => {
                    copy_mapped(&mut locale.region, subtag, |b| *b);
                }
                1 => break,
                _ => {}
            }
        }
        Some(locale)
    }

    pub fn from_env() -> Option<Self> {
        crate::system::locale()
    }

    pub fn language(&self) -> &str {
        subtag(&self.language)
    }

    pub fn script(&self) -> Option<&str> {
        Some(subtag(&self.script)).filter(|script| !script.is_empty())
    }

    pub fn region(&self) -> Option<&str> {
        Some(subtag(&self.region)).filter(|region| !region.is_empty())
    }

    pub(crate) fn key(&self) -> [u8; 10] {
        let mut key = [0; 10];
        key[..3].copy_from_slice(&self.language);
        key[3..7].copy_from_slice(&self.script);
        key[7..].copy_from_slice(&self.region);
        key
    }
}

fn copy_mapped(target: &mut [u8], source: &str, map: impl Fn(&u8) -> u8) {
    for (slot, byte) in target.iter_mut().zip(source.as_bytes()) {
        *slot = map(byte);
    }
}

fn subtag(bytes: &[u8]) -> &str {
    let length = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..length]).unwrap_or_default()
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.language())?;
        if let Some(script) = self.script() {
            write!(f, "-{script}")?;
        }
        if let Some(region) = self.region() {
            write!(f, "-{region}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Locale({self})")
    }
}
