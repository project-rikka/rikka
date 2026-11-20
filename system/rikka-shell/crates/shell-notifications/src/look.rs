use jiff::civil::Date;
use rikka_notifications::{Image, Notification};
use rikka_ui::{IconSource, ImageSource};
use rikka_xdg::Catalog;
use shell_apps::{Apps, owner};

#[derive(Clone, Debug)]
pub struct Look {
    pub(crate) icon: IconSource,
    pub(crate) at: String,
    pub(crate) on: Option<Date>,
    pub(crate) buttons: Vec<Button>,
}

#[derive(Clone, Debug)]
pub(crate) struct Button {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) icon: Option<IconSource>,
}

impl Look {
    #[must_use]
    pub(super) fn for_notification(notification: &Notification, apps: &Apps) -> Look {
        let shown_text = jiff::Zoned::try_from(notification.arrived).ok();
        Look {
            icon: icon(notification, apps),
            at: shown_text
                .as_ref()
                .map(|zoned| zoned.strftime("%H:%M").to_string())
                .unwrap_or_default(),
            on: shown_text.as_ref().map(jiff::Zoned::date),
            buttons: notification
                .buttons()
                .map(|action| Button {
                    key: action.key.clone(),
                    label: action.label.clone(),
                    icon: notification
                        .action_icons
                        .then(|| apps.icons().named(&action.key))
                        .flatten(),
                })
                .collect(),
        }
    }
}

fn icon(notification: &Notification, apps: &Apps) -> IconSource {
    match &notification.image {
        Some(Image::Pixels(picture)) => IconSource::from(ImageSource::Rgba {
            width: picture.width,
            height: picture.height,
            pixels: picture.pixels.clone(),
        }),
        Some(Image::File(path)) => IconSource::from(ImageSource::file(path.as_path())),
        Some(Image::Named(name)) => apps
            .icons()
            .named(name)
            .unwrap_or_else(|| application_icon(notification, apps)),
        None => application_icon(notification, apps),
    }
}

fn application_icon(notification: &Notification, apps: &Apps) -> IconSource {
    apps.installed()
        .with(|catalogue| apps.icons().icon_for(entry(notification, catalogue)))
}

fn entry<'a>(
    notification: &Notification,
    catalogue: &'a Catalog,
) -> Option<&'a rikka_xdg::DesktopEntry> {
    let entries = catalogue.entries();
    notification
        .desktop_entry
        .as_deref()
        .and_then(|named| owner(entries, named))
        .or_else(|| owner(entries, &notification.app_name))
}
