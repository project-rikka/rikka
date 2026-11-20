use rikka_proton_mail::LabelId;

use crate::folder::Kind;

pub const PAGE: u32 = 50;

#[must_use]
#[derive(Clone, Debug)]
pub struct Ask {
    pub folder: LabelId,
    pub kind: Kind,
    pub grouped: bool,
    pub keyword: Option<String>,
    pub unread: bool,
    pub page: u32,
    pub size: u32,
}

impl Ask {
    pub fn new(folder: LabelId, kind: Kind, grouped: bool) -> Ask {
        Ask {
            folder,
            kind,
            grouped,
            keyword: None,
            unread: false,
            page: 0,
            size: PAGE,
        }
    }

    pub fn looking_for(mut self, keyword: impl Into<String>) -> Ask {
        let keyword = keyword.into();
        self.keyword = (!keyword.trim().is_empty()).then_some(keyword);
        self
    }

    pub fn unread_only(mut self, unread: bool) -> Ask {
        self.unread = unread;
        self
    }

    pub fn at_page(mut self, page: u32) -> Ask {
        self.page = page;
        self
    }

    #[must_use]
    pub fn recipients(&self) -> bool {
        self.kind.shows_recipients()
    }

    #[must_use]
    pub fn is_first(&self) -> bool {
        self.page == 0
    }

    #[must_use]
    pub fn threaded(&self) -> bool {
        self.grouped && !self.kind.holds_drafts()
    }
}
