use crate::chrome::Chrome;
use crate::default_browser::DefaultBrowser;
use crate::page_services::PageServices;
use crate::search::Searching;
use crate::translate::Translations;

#[derive(Debug, Default)]
pub(crate) struct Services {
    pub(crate) search: Searching,
    pub(crate) page_services: PageServices,
    pub(crate) translations: Translations,
    pub(crate) default_browser: DefaultBrowser,
    pub(crate) chrome: Chrome,
}
