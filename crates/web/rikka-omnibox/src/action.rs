use url::Url;

use crate::search::Search;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action<K> {
    Navigate(Url),
    Search(Search),
    SwitchToTab(K),
    RestoreArchived(K),
    RunCommand(K),
}
