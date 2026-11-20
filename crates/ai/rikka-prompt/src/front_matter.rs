#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontMatter<'a> {
    Absent,
    Unclosed,
    Fenced { front: &'a str, body: &'a str },
}

impl<'a> FrontMatter<'a> {
    #[must_use]
    pub fn split(text: &'a str, fence: &str) -> FrontMatter<'a> {
        let opened = text.strip_prefix(fence).and_then(|rest| {
            rest.strip_prefix("\r\n")
                .or_else(|| rest.strip_prefix('\n'))
        });
        let Some(rest) = opened else {
            return FrontMatter::Absent;
        };

        let mut at = 0;
        for line in rest.split_inclusive('\n') {
            if line.trim_end() == fence {
                return FrontMatter::Fenced {
                    front: &rest[..at],
                    body: &rest[at + line.len()..],
                };
            }
            at += line.len();
        }
        FrontMatter::Unclosed
    }
}
