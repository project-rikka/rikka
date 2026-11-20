use rikka_ui::DragOffer;

use browser_model::{FavoriteId, FolderId, PinnedItem, PinnedTabId, Row, TabId};

const KIND: &str = "application/x-rikka-browser-row";

pub(crate) fn mime() -> String {
    format!("{KIND};pid={}", std::process::id())
}

pub(crate) fn encode(row: Row) -> Vec<u8> {
    let (kind, id) = match row {
        Row::Tab(tab) => ("tab", tab.get()),
        Row::Pin(PinnedItem::Tab(pin)) => ("pin", pin.get()),
        Row::Pin(PinnedItem::Folder(folder)) => ("folder", folder.get()),
        Row::Favorite(favorite) => ("favorite", favorite.get()),
    };
    format!("{kind}:{id}").into_bytes()
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Row> {
    let text = std::str::from_utf8(bytes).ok()?;
    let (kind, id) = text.split_once(':')?;
    let id = id.parse().ok()?;
    match kind {
        "tab" => Some(Row::Tab(TabId::from_raw(id))),
        "pin" => Some(Row::Pin(PinnedItem::Tab(PinnedTabId::from_raw(id)))),
        "folder" => Some(Row::Pin(PinnedItem::Folder(FolderId::from_raw(id)))),
        "favorite" => Some(Row::Favorite(FavoriteId::from_raw(id))),
        _ => None,
    }
}

pub(crate) fn offered(offer: &DragOffer) -> bool {
    offer.offers(&mime())
}

pub(crate) fn read(offer: &DragOffer) -> Option<Row> {
    decode(&offer.read(&mime())?)
}
