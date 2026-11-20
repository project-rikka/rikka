#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dialect {
    pub(crate) id: &'static str,
    quoted: bool,
    keep: &'static str,
}

pub(crate) const ELEMENT_TAG: &str = "a";
pub(crate) const KEEP_TAG: &str = "code";
pub(crate) const PRE_TAG: &str = "pre";

impl Dialect {
    #[must_use]
    pub const fn new(id: &'static str) -> Dialect {
        assert!(!id.is_empty(), "dialect id attribute must not be empty");
        Dialect {
            id,
            quoted: false,
            keep: "",
        }
    }

    #[must_use]
    pub const fn quoted(self) -> Dialect {
        Dialect {
            quoted: true,
            ..self
        }
    }

    #[must_use]
    pub const fn keep(self, attributes: &'static str) -> Dialect {
        Dialect {
            keep: attributes,
            ..self
        }
    }

    pub(crate) fn open_element(&self, out: &mut String, id: u32) {
        self.open(out, ELEMENT_TAG, id, "");
    }

    pub(crate) fn open_keep(&self, out: &mut String, id: u32) {
        self.open(out, KEEP_TAG, id, self.keep);
    }

    fn open(&self, out: &mut String, tag: &str, id: u32, extra: &str) {
        out.push('<');
        out.push_str(tag);
        out.push(' ');
        out.push_str(self.id);
        out.push('=');
        if self.quoted {
            out.push('"');
            out.push_str(&id.to_string());
            out.push('"');
        } else {
            out.push_str(&id.to_string());
        }
        if !extra.is_empty() {
            out.push(' ');
            out.push_str(extra);
        }
        out.push('>');
    }
}

pub(crate) fn close(out: &mut String, tag: &str) {
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}
