use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::NotificationId;

wire_enum! {
    pub enum NotificationState: default Unread {
        Unread = 0,
        Read = 1,
        Dismissed = 2,
    }
}

wire_enum! {
    pub enum NotificationDisplay: default Banner {
        Banner = 0,
        Modal = 1,
        Promo = 2,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationCta {
    #[serde(rename = "Text", default)]
    text: String,
    #[serde(rename = "Type", default)]
    kind: String,
    #[serde(rename = "Ref", default)]
    reference: String,
}

impl NotificationCta {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn is_external_link(&self) -> bool {
        self.kind == "external_link"
    }

    #[must_use]
    pub fn is_internal_navigation(&self) -> bool {
        self.kind == "internal_navigation"
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTheme {
    #[serde(rename = "BackgroundImageUrl", default)]
    background_image_url: String,
    #[serde(rename = "ContentImageUrl", default)]
    content_image_url: String,
    #[serde(rename = "ClosePromoTextColor", default)]
    close_text_color: String,
}

impl NotificationTheme {
    #[must_use]
    pub fn background_image_url(&self) -> &str {
        &self.background_image_url
    }

    #[must_use]
    pub fn content_image_url(&self) -> &str {
        &self.content_image_url
    }

    #[must_use]
    pub fn close_text_color(&self) -> &str {
        &self.close_text_color
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPromo {
    #[serde(rename = "StartMinimized", default)]
    start_minimized: bool,
    #[serde(rename = "ClosePromoText", default)]
    close_text: String,
    #[serde(rename = "MinimizedPromoText", default)]
    minimized_text: String,
    #[serde(rename = "LightThemeContents")]
    light: NotificationTheme,
    #[serde(rename = "DarkThemeContents")]
    dark: NotificationTheme,
}

impl NotificationPromo {
    #[must_use]
    pub fn starts_minimized(&self) -> bool {
        self.start_minimized
    }

    #[must_use]
    pub fn close_text(&self) -> &str {
        &self.close_text
    }

    #[must_use]
    pub fn minimized_text(&self) -> &str {
        &self.minimized_text
    }

    #[must_use]
    pub fn light(&self) -> &NotificationTheme {
        &self.light
    }

    #[must_use]
    pub fn dark(&self) -> &NotificationTheme {
        &self.dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationContent {
    #[serde(rename = "ImageUrl", default)]
    image_url: Option<String>,
    #[serde(rename = "DisplayType", default)]
    display: NotificationDisplay,
    #[serde(rename = "Title", default)]
    title: String,
    #[serde(rename = "Message", default)]
    message: String,
    #[serde(rename = "Theme", default)]
    theme: Option<String>,
    #[serde(rename = "Cta", default)]
    cta: Option<NotificationCta>,
    #[serde(rename = "PromoContents", default)]
    promo: Option<NotificationPromo>,
}

impl NotificationContent {
    #[must_use]
    pub fn image_url(&self) -> Option<&str> {
        self.image_url.as_deref()
    }

    #[must_use]
    pub fn display(&self) -> NotificationDisplay {
        self.display
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn theme(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    #[must_use]
    pub fn cta(&self) -> Option<&NotificationCta> {
        self.cta.as_ref()
    }

    #[must_use]
    pub fn promo(&self) -> Option<&NotificationPromo> {
        self.promo.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(rename = "ID")]
    id: NotificationId,
    #[serde(rename = "NotificationKey", default)]
    key: String,
    #[serde(rename = "StartTime", default, with = "rikka_proton_api::unix_seconds")]
    start_time: Timestamp,
    #[serde(
        rename = "EndTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    end_time: Option<Timestamp>,
    #[serde(rename = "State", default)]
    state: NotificationState,
    #[serde(rename = "Priority", default)]
    priority: i64,
    #[serde(rename = "Content")]
    content: NotificationContent,
}

impl Notification {
    #[must_use]
    pub fn id(&self) -> &NotificationId {
        &self.id
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    #[must_use]
    pub fn end_time(&self) -> Option<Timestamp> {
        self.end_time
    }

    #[must_use]
    pub fn state(&self) -> NotificationState {
        self.state
    }

    #[must_use]
    pub fn priority(&self) -> i64 {
        self.priority
    }

    #[must_use]
    pub fn content(&self) -> &NotificationContent {
        &self.content
    }

    #[must_use]
    pub fn is_shown_at(&self, at: Timestamp) -> bool {
        self.state == NotificationState::Unread
            && self.start_time <= at
            && self.end_time.is_none_or(|end| at < end)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct NotificationPage {
    #[serde(rename = "Notifications", default)]
    pub(crate) notifications: Vec<Notification>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct NotificationStateRequest {
    #[serde(rename = "State")]
    pub(crate) state: NotificationState,
}
