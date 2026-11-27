use rikka_app::prelude::*;

use crate::home::HomePage;
use crate::internal::archive::ArchivePage;
use crate::internal::clearing::Clearing;
use crate::internal::downloads::DownloadsPage;
use crate::internal::history::HistoryPage;

#[derive(State, Debug)]
pub(crate) struct Pages {
    #[state]
    pub(crate) home: HomePage,
    #[state]
    pub(crate) history: HistoryPage,
    #[state]
    pub(crate) archive: ArchivePage,
    #[state]
    pub(crate) clearing: Clearing,
    #[state]
    pub(crate) downloads: DownloadsPage,
}

impl Pages {
    pub(crate) fn new() -> Pages {
        Pages {
            home: HomePage::new(),
            history: HistoryPage::new(),
            archive: ArchivePage::new(),
            clearing: Clearing::new(),
            downloads: DownloadsPage::new(),
        }
    }
}
