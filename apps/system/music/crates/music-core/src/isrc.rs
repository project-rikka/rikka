use std::fmt;

const LENGTH: usize = 12;
const COUNTRY: usize = 2;
const REGISTRANT: usize = 5;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Isrc([u8; LENGTH]);

impl Isrc {
    #[must_use]
    pub fn parse(text: &str) -> Option<Isrc> {
        let mut code = [0; LENGTH];
        let mut filled = 0;
        for byte in text.trim().bytes().filter(|byte| *byte != b'-') {
            let slot = code.get_mut(filled)?;
            *slot = byte.to_ascii_uppercase();
            filled += 1;
        }
        let fits = filled == LENGTH
            && code[..COUNTRY].iter().all(u8::is_ascii_uppercase)
            && code[COUNTRY..REGISTRANT]
                .iter()
                .all(u8::is_ascii_alphanumeric)
            && code[REGISTRANT..].iter().all(u8::is_ascii_digit);
        fits.then_some(Isrc(code))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
}

impl fmt::Debug for Isrc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for Isrc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
