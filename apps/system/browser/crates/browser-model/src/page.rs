pub trait LivePage {
    type View: Clone;

    fn view(&self) -> &Self::View;
}

#[derive(Debug, Default)]
pub enum Page<L> {
    #[default]
    Unloaded,
    Live(L),
}

impl<L> Page<L> {
    pub fn live(&self) -> Option<&L> {
        match self {
            Page::Live(live) => Some(live),
            Page::Unloaded => None,
        }
    }

    pub fn live_mut(&mut self) -> Option<&mut L> {
        match self {
            Page::Live(live) => Some(live),
            Page::Unloaded => None,
        }
    }
}

impl<L: LivePage> Page<L> {
    pub fn view(&self) -> Option<&L::View> {
        self.live().map(LivePage::view)
    }
}
