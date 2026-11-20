use std::fmt;

pub(crate) struct Masked<'a>(pub(crate) &'a str);

impl fmt::Debug for Masked<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.is_empty() {
            true => f.write_str("\"\""),
            false => write!(f, "\"<{} hidden bytes>\"", self.0.len()),
        }
    }
}
