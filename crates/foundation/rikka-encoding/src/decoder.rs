use encoding_rs::{Encoding, UTF_8};

use crate::guess::guess;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    pub encoding: &'static Encoding,
    pub bom: bool,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default)]
pub struct Decoder<'a> {
    declared: Option<&'static Encoding>,
    top_level_domain: Option<&'a str>,
    truncated: bool,
}

impl<'a> Decoder<'a> {
    pub fn new() -> Decoder<'a> {
        Decoder::default()
    }

    pub fn declared(mut self, encoding: impl Into<Option<&'static Encoding>>) -> Decoder<'a> {
        self.declared = encoding.into();
        self
    }

    pub fn top_level_domain(mut self, domain: impl Into<Option<&'a str>>) -> Decoder<'a> {
        self.top_level_domain = domain.into();
        self
    }

    pub fn truncated(mut self, truncated: bool) -> Decoder<'a> {
        self.truncated = truncated;
        self
    }

    #[must_use]
    pub fn decode(self, bytes: impl Into<Vec<u8>>) -> Decoded {
        let bytes = bytes.into();
        if let Some((encoding, mark)) = Encoding::for_bom(&bytes) {
            return decoded_as(encoding, &bytes[mark..], true);
        }
        if let Some(encoding) = self.declared {
            return decoded_as(encoding, &bytes, false);
        }
        let bytes = match String::from_utf8(bytes) {
            Ok(text) => {
                return Decoded {
                    text,
                    encoding: UTF_8,
                    bom: false,
                };
            }
            Err(error) if self.truncated && error.utf8_error().error_len().is_none() => {
                return Decoded {
                    text: String::from_utf8_lossy(error.as_bytes()).into_owned(),
                    encoding: UTF_8,
                    bom: false,
                };
            }
            Err(error) => error.into_bytes(),
        };
        let encoding = guess([bytes.as_slice()], self.top_level_domain);
        decoded_as(encoding, &bytes, false)
    }
}

#[must_use]
pub fn decode(bytes: impl Into<Vec<u8>>) -> Decoded {
    Decoder::new().decode(bytes)
}

fn decoded_as(encoding: &'static Encoding, bytes: &[u8], bom: bool) -> Decoded {
    Decoded {
        text: encoding.decode_without_bom_handling(bytes).0.into_owned(),
        encoding,
        bom,
    }
}
