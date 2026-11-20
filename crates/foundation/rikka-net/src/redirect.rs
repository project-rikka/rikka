#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redirect {
    None,
    Limited(usize),
}

impl Redirect {
    pub(crate) fn policy(self) -> reqwest::redirect::Policy {
        match self {
            Redirect::None => reqwest::redirect::Policy::none(),
            Redirect::Limited(limit) => reqwest::redirect::Policy::limited(limit),
        }
    }
}

impl Default for Redirect {
    fn default() -> Redirect {
        Redirect::Limited(10)
    }
}
