use rikka_tray::{Icon, Item};
use rikka_ui::{IconSource, ImageSource};
use shell_apps::{AppIcons, Apps, DesktopEntry, owner};

#[derive(Clone, Debug)]
pub(crate) struct Look {
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) icon: IconSource,
}

impl Look {
    #[must_use]
    pub(super) fn for_item(item: &Item, apps: &Apps) -> Look {
        let application = apps
            .installed()
            .with(|catalogue| entry(catalogue.entries(), item).cloned());
        let label = label(item, application.as_ref());
        Look {
            detail: detail(item, &label),
            icon: source(item, application.as_ref(), apps.icons()),
            label,
        }
    }
}

fn label(item: &Item, application: Option<&DesktopEntry>) -> String {
    if let Some(name) = item.accessible_name() {
        return name.to_owned();
    }
    match application {
        Some(entry) => entry.name.clone(),
        None => item.label().to_owned(),
    }
}

fn detail(item: &Item, label: &str) -> Option<String> {
    let tooltip = item.tooltip.as_ref()?;
    let detail = match tooltip.description.trim() {
        "" => tooltip.title.trim(),
        description => description,
    };
    (!detail.is_empty() && detail != label).then(|| detail.to_owned())
}

fn source(item: &Item, application: Option<&DesktopEntry>, icons: &AppIcons) -> IconSource {
    artwork(item.icon.as_ref(), icons).unwrap_or_else(|| icons.icon_for(application))
}

#[must_use]
pub(crate) fn artwork(icon: Option<&Icon>, icons: &AppIcons) -> Option<IconSource> {
    match icon? {
        Icon::Pixels(picture) => Some(IconSource::from(ImageSource::Rgba {
            width: picture.width,
            height: picture.height,
            pixels: picture.pixels.clone(),
        })),
        Icon::Bytes(bytes) => Some(IconSource::from(ImageSource::Bytes(bytes.clone()))),
        Icon::Named { name, theme_path } => theme_path
            .as_deref()
            .and_then(|root| icons.shipped(root, name))
            .or_else(|| icons.named(name)),
    }
}

fn entry<'a>(entries: &'a [DesktopEntry], item: &Item) -> Option<&'a DesktopEntry> {
    item.process
        .as_deref()
        .and_then(|program| owner(entries, program))
        .or_else(|| owner(entries, &item.name))
}
