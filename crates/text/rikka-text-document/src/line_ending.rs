use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
    Cr,
}

impl LineEnding {
    pub fn detect(text: &str) -> Self {
        match text.find(['\n', '\r']) {
            Some(at) if text.as_bytes()[at] == b'\n' => Self::Lf,
            Some(at) if text.as_bytes().get(at + 1) == Some(&b'\n') => Self::Crlf,
            Some(_) => Self::Cr,
            None => Self::Lf,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }

    pub fn join(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Lf => Cow::Borrowed(text),
            _ => Cow::Owned(text.replace('\n', self.as_str())),
        }
    }

    pub fn normalize(text: &str) -> Cow<'_, str> {
        if text.contains('\r') {
            Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
        } else {
            Cow::Borrowed(text)
        }
    }
}
