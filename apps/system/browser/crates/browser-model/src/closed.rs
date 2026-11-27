use browser_store::{ArchiveScope, ClosedTab, Restored, Store};

use crate::ids::{SpaceId, TabId};
use crate::saved::Saved;

#[derive(Clone, Debug, PartialEq)]
pub struct Closed {
    pub tab: TabId,
    pub space: Option<SpaceId>,
    pub saved: Saved,
}

const SAME_MOMENT: i64 = 2_000;
const LOOKED_AT: usize = 8;

pub fn take_closed(store: &Store) -> browser_store::Result<Option<Restored>> {
    let Some((closed, restored)) = store.pop_closed_tab()? else {
        return Ok(None);
    };
    forget_archived(store, &closed)?;
    Ok(Some(restored))
}

pub fn take_closed_tab(
    store: &Store,
    closed: &ClosedTab,
) -> browser_store::Result<Option<Restored>> {
    let Some(restored) = store.take_closed_tab(closed.id)? else {
        return Ok(None);
    };
    forget_archived(store, closed)?;
    Ok(Some(restored))
}

fn forget_archived(store: &Store, closed: &ClosedTab) -> browser_store::Result<()> {
    let archived = store
        .search_archive(ArchiveScope::Space(closed.space), &closed.url, LOOKED_AT)?
        .into_iter()
        .find(|entry| {
            entry.url == closed.url
                && (entry.archived_at.unix_millis() - closed.closed_at.unix_millis()).abs()
                    <= SAME_MOMENT
        });
    if let Some(entry) = archived {
        store.remove_archived(entry.id);
    }
    Ok(())
}
