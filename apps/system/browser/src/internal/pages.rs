use rikka_app::prelude::*;

use crate::home::HomePage;
use crate::internal::archive::ArchivePage;
use crate::internal::clearing::Clearing;
use crate::internal::downloads::DownloadsPage;
use crate::internal::history::HistoryPage;
use crate::internal::sites::SitesPage;
use crate::internal::source::SourcePage;

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
    #[state]
    pub(crate) source: SourcePage,
    #[state]
    pub(crate) sites: SitesPage,
}

impl Pages {
    pub(crate) fn new() -> Pages {
        Pages {
            home: HomePage::new(),
            history: HistoryPage::new(),
            archive: ArchivePage::new(),
            clearing: Clearing::new(),
            downloads: DownloadsPage::new(),
            source: SourcePage::new(),
            sites: SitesPage::new(),
        }
    }
}
