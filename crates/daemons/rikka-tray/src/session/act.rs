use zbus::blocking::Connection;

use crate::item::Item;
use crate::menu::Menu;
use crate::proxies::ItemProxy;
use crate::request::Request;
use crate::session::menu;

pub(crate) fn perform(connection: &Connection, item: &Item, request: &Request) -> Option<Menu> {
    match request {
        Request::Menu(_) => return menu::read(connection, item),
        Request::Choose { row, .. } => menu::send_event(connection, item, *row, "clicked"),
        Request::Closed(_) => menu::send_event(connection, item, ROOT, "closed"),
        press => send(connection, item, press),
    }
    None
}

const ROOT: i32 = 0;

fn send(connection: &Connection, item: &Item, request: &Request) {
    let Some(proxy) = proxy(connection, item) else {
        return;
    };
    let sent = match request {
        Request::Activate { at, .. } => proxy.activate(at.0, at.1),
        Request::Secondary { at, .. } => proxy.secondary_activate(at.0, at.1),
        Request::ContextMenu { at, .. } => proxy.context_menu(at.0, at.1),
        Request::Scroll { delta, .. } => {
            let (delta, orientation) = delta.parts();
            proxy.scroll(delta, orientation)
        }
        Request::Menu(_) | Request::Choose { .. } | Request::Closed(_) => return,
    };
    if let Err(error) = sent {
        tracing::debug!("tray item {} rejected activation: {error}", item.id);
    }
}

fn proxy<'a>(connection: &'a Connection, item: &Item) -> Option<ItemProxy<'a>> {
    ItemProxy::builder(connection)
        .destination(item.id.service.clone())
        .ok()?
        .path(item.id.path.clone())
        .ok()?
        .build()
        .ok()
}
