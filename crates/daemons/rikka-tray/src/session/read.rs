use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Type, Value};

use crate::category::Category;
use crate::icon::{Icon, RawPixmap};
use crate::id::ItemId;
use crate::item::Item;
use crate::proxies::{ITEM, ITEM_ALTERNATIVE};
use crate::session::programs::Programs;
use crate::status::Status;
use crate::tooltip::Tooltip;
use rikka_bus::Properties;

#[derive(Clone, Debug, Type, Value, OwnedValue)]
struct RawTooltip {
    icon_name: String,
    icon_pixmap: Vec<RawPixmap>,
    title: String,
    description: String,
}

const ICON: (&str, &str) = ("IconName", "IconPixmap");
const ATTENTION: (&str, &str) = ("AttentionIconName", "AttentionIconPixmap");
const OVERLAY: (&str, &str) = ("OverlayIconName", "OverlayIconPixmap");

#[must_use]
pub(crate) fn item(connection: &Connection, programs: &mut Programs, id: &ItemId) -> Option<Item> {
    let properties = all(connection, id)?;
    let status = Status::named(properties.string_ref("Status").unwrap_or_default());
    let theme_path = properties.string_ref("IconThemePath");

    Some(Item {
        id: id.clone(),
        name: properties.string("Id").unwrap_or_default(),
        title: properties.string("Title").unwrap_or_default(),
        category: Category::named(properties.string_ref("Category").unwrap_or_default()),
        status,
        icon: icon(&properties, status, theme_path),
        overlay: artwork(&properties, OVERLAY, theme_path),
        tooltip: tooltip(&properties, theme_path),
        is_menu: properties.boolean("ItemIsMenu").unwrap_or(false),
        menu: properties.object_path("Menu").map(str::to_owned),
        process: programs.resolve(connection, &id.service),
    })
}

fn all(connection: &Connection, id: &ItemId) -> Option<Properties> {
    [ITEM, ITEM_ALTERNATIVE].into_iter().find_map(|interface| {
        Properties::read(connection, id.service.as_str(), id.path.as_str(), interface)
            .inspect_err(|error| tracing::trace!("failed to read {interface} from {id}: {error}"))
            .ok()
            .filter(|fetched| !fetched.is_empty())
    })
}

fn icon(properties: &Properties, status: Status, theme_path: Option<&str>) -> Option<Icon> {
    let ordinary = || artwork(properties, ICON, theme_path);
    match status.needs_attention() {
        true => artwork(properties, ATTENTION, theme_path).or_else(ordinary),
        false => ordinary(),
    }
}

fn artwork(
    properties: &Properties,
    (name, pixmap): (&str, &str),
    theme_path: Option<&str>,
) -> Option<Icon> {
    Icon::best(
        properties.string_ref(name).unwrap_or_default(),
        &properties.list::<RawPixmap>(pixmap).unwrap_or_default(),
        theme_path,
    )
}

fn tooltip(properties: &Properties, theme_path: Option<&str>) -> Option<Tooltip> {
    let raw = properties.value::<RawTooltip>("ToolTip")?;
    let tooltip = Tooltip {
        icon: Icon::best(&raw.icon_name, &raw.icon_pixmap, theme_path),
        title: raw.title,
        description: raw.description,
    };
    (!tooltip.is_empty()).then_some(tooltip)
}
