use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

use crate::icon::Icon;
use crate::item::Item;
use crate::menu::{self, Entry, Menu, Row, Toggle};
use crate::proxies::{MenuProxy, RawLayout};
use rikka_bus::Properties;

const DEEPEST: u8 = 8;

#[must_use]
pub(crate) fn read(connection: &Connection, item: &Item) -> Option<Menu> {
    let proxy = proxy(connection, item)?;
    if let Err(error) = proxy.about_to_show(ROOT) {
        tracing::debug!(%error, item = %item.id, "AboutToShow failed");
    }
    let (_, root) = proxy
        .get_layout(ROOT, WHOLE_TREE, &[])
        .inspect_err(|error| tracing::debug!("failed to open menu of {}: {error}", item.id))
        .ok()?;

    send_event(connection, item, ROOT, "opened");
    Some(Menu {
        item: item.id.clone(),
        entries: rows(&proxy, &root.children, DEEPEST),
    })
}

pub(crate) fn send_event(connection: &Connection, item: &Item, row: i32, event_id: &str) {
    let Some(proxy) = proxy(connection, item) else {
        return;
    };
    if let Err(error) = proxy.event(row, event_id, &Value::from(""), stamp()) {
        tracing::debug!(
            "failed to send menu event `{event_id}` to {}: {error}",
            item.id
        );
    }
}

const WHOLE_TREE: i32 = -1;

const ROOT: i32 = 0;

fn proxy<'a>(connection: &'a Connection, item: &Item) -> Option<MenuProxy<'a>> {
    MenuProxy::builder(connection)
        .destination(item.id.service.clone())
        .ok()?
        .path(item.menu.clone()?)
        .ok()?
        .build()
        .ok()
}

fn rows(proxy: &MenuProxy<'_>, children: &[OwnedValue], left: u8) -> Vec<Entry> {
    children
        .iter()
        .filter_map(|child| entry(proxy, child, left))
        .collect()
}

fn entry(proxy: &MenuProxy<'_>, child: &OwnedValue, left: u8) -> Option<Entry> {
    let RawLayout {
        id,
        properties,
        children,
    } = child
        .try_clone()
        .ok()
        .and_then(|value| RawLayout::try_from(value).ok())?;
    let properties = Properties::from(properties);

    if !properties.boolean("visible").unwrap_or(true) {
        return None;
    }
    if properties.string_ref("type") == Some("separator") {
        return Some(Entry::Separator);
    }

    Some(Entry::Row(Row {
        id,
        label: menu::plain(properties.string_ref("label").unwrap_or_default()),
        enabled: properties.boolean("enabled").unwrap_or(true),
        icon: icon(&properties),
        toggle: toggle(&properties),
        entries: submenu(proxy, id, &properties, &children, left),
    }))
}

fn submenu(
    proxy: &MenuProxy<'_>,
    id: i32,
    properties: &Properties,
    children: &[OwnedValue],
    left: u8,
) -> Vec<Entry> {
    if left == 0 {
        return Vec::new();
    }
    if !children.is_empty() {
        return rows(proxy, children, left - 1);
    }
    if properties.string_ref("children-display") != Some("submenu") {
        return Vec::new();
    }

    if let Err(error) = proxy.about_to_show(id) {
        tracing::debug!(%error, id, "AboutToShow failed");
    }
    let Ok((_, layout)) = proxy.get_layout(id, WHOLE_TREE, &[]) else {
        return Vec::new();
    };
    rows(proxy, &layout.children, left - 1)
}

fn icon(properties: &Properties) -> Option<Icon> {
    let named = properties
        .string_ref("icon-name")
        .map(str::trim)
        .filter(|name| !name.is_empty());
    if let Some(name) = named {
        return Some(Icon::Named {
            name: name.to_owned(),
            theme_path: None,
        });
    }
    let data = properties.list::<u8>("icon-data").unwrap_or_default();
    (!data.is_empty()).then(|| Icon::Bytes(data.into()))
}

fn toggle(properties: &Properties) -> Option<Toggle> {
    let on = properties.value::<i32>("toggle-state") == Some(1);
    match properties.string_ref("toggle-type")? {
        "checkmark" => Some(Toggle::Check(on)),
        "radio" => Some(Toggle::Radio(on)),
        _ => None,
    }
}

fn stamp() -> u32 {
    std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .map(|since| since.as_secs() as u32)
        .unwrap_or_default()
}
