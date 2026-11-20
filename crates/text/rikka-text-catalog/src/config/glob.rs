#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Glob(Box<[u8]>);

impl Glob {
    pub(crate) fn new(pattern: &str) -> Self {
        Self(pattern.as_bytes().into())
    }

    pub(crate) fn matches(&self, text: &[u8]) -> bool {
        let pattern = &self.0;
        let (mut p, mut t) = (0, 0);
        let mut resume: Option<(usize, usize)> = None;
        while t < text.len() {
            match pattern.get(p) {
                Some(b'*') => {
                    resume = Some((p, t));
                    p += 1;
                }
                Some(&expected) if expected == b'?' || expected == text[t] => {
                    p += 1;
                    t += 1;
                }
                _ => match resume {
                    Some((star, consumed)) => {
                        p = star + 1;
                        t = consumed + 1;
                        resume = Some((star, consumed + 1));
                    }
                    None => return false,
                },
            }
        }
        pattern[p..].iter().all(|&byte| byte == b'*')
    }
}
