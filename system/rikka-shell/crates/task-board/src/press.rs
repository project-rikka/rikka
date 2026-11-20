use crate::board::{Group, Window};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Focus(String),
    Minimize(String),
}

impl Group {
    #[must_use]
    pub fn press(&self) -> Option<Act> {
        match self.windows.as_slice() {
            [] => None,
            [window] if window.focused && !window.minimized => {
                (!window.tiled).then(|| Act::Minimize(window.id.clone()))
            }
            [window] => Some(Act::Focus(window.id.clone())),
            windows => {
                let next = match windows.iter().position(|window| window.focused) {
                    Some(focused) => &windows[(focused + 1) % windows.len()],
                    None => self.recent()?,
                };
                Some(Act::Focus(next.id.clone()))
            }
        }
    }

    #[must_use]
    pub fn recent(&self) -> Option<&Window> {
        let latest = |minimized: bool| {
            self.windows
                .iter()
                .filter(move |window| window.minimized == minimized)
                .max_by_key(|window| window.recency)
        };
        latest(false).or_else(|| latest(true))
    }
}
