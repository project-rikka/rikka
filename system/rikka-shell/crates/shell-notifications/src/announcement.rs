use std::collections::HashMap;
use std::path::Path;

use rikka_notifications::{DEFAULT_ACTION, Image, Notification, Progress, Timeout, Urgency};
use rikka_seam::{Request, SeamClient};
use rikka_ui::tr;
use serde::{Deserialize, Serialize};

use crate::action_request::ActionRequest;
use crate::service::{CAPABILITY, POST};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Announcement {
    pub(crate) app_name: String,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) image: String,
    pub(crate) urgent: bool,
    pub(crate) resident: bool,
    pub(crate) tag: Option<String>,
    pub(crate) progress: Option<Progress>,
    pub(crate) action_request: Option<ActionRequest>,
    pub(crate) buttons: Vec<AnnouncedButton>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AnnouncedButton {
    label: String,
    action_request: ActionRequest,
}

pub(super) struct Announced {
    pub(super) tag: Option<String>,
    pub(super) notification: Notification,
    pub(super) action_requests: HashMap<String, ActionRequest>,
}

impl Announcement {
    #[must_use]
    pub fn new(app_name: impl Into<String>, summary: impl Into<String>) -> Announcement {
        Announcement {
            app_name: app_name.into(),
            summary: summary.into(),
            ..Announcement::default()
        }
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<String>) -> Announcement {
        self.body = body.into();
        self
    }

    #[must_use]
    pub fn file(self, file: &Path) -> Announcement {
        let shown = match rikka_dirs::home().map(|home| file.strip_prefix(home)) {
            Some(Ok(rest)) => format!("~/{}", rest.display()),
            _ => file.display().to_string(),
        };
        self.body(shown)
    }

    #[must_use]
    pub fn image(mut self, image: impl Into<String>) -> Announcement {
        self.image = image.into();
        self
    }

    #[must_use]
    pub fn urgent(mut self) -> Announcement {
        self.urgent = true;
        self
    }

    #[must_use]
    pub fn resident(mut self) -> Announcement {
        self.resident = true;
        self
    }

    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Announcement {
        self.tag = Some(tag.into());
        self
    }

    #[must_use]
    pub fn progress(mut self, progress: Progress) -> Announcement {
        self.progress = Some(progress);
        self
    }

    #[must_use]
    pub fn action_request(mut self, action_request: ActionRequest) -> Announcement {
        self.action_request = Some(action_request);
        self
    }

    #[must_use]
    pub fn button(
        mut self,
        label: impl Into<String>,
        action_request: ActionRequest,
    ) -> Announcement {
        self.buttons.push(AnnouncedButton {
            label: label.into(),
            action_request,
        });
        self
    }

    pub fn send(self, seam: &SeamClient) {
        seam.notify(Request::new(CAPABILITY, POST).encode(&self));
    }

    #[must_use]
    pub(super) fn into_announced(self) -> Announced {
        let mut notification = Notification::new(self.app_name, self.summary)
            .body(self.body)
            .urgency(match self.urgent {
                true => Urgency::Critical,
                false => Urgency::Normal,
            })
            .timeout(Timeout::Default);
        if self.resident {
            notification = notification.resident();
        }
        if let Some(progress) = self.progress {
            notification = notification.progress(progress);
        }
        if let Some(image) = Image::named(&self.image) {
            notification = notification.image(image);
        }

        let mut action_requests = HashMap::new();
        if let Some(action_request) = self.action_request {
            notification =
                notification.action(DEFAULT_ACTION, tr!("shell.notifications.open").to_string());
            action_requests.insert(DEFAULT_ACTION.to_owned(), action_request);
        }
        for (index, button) in self.buttons.into_iter().enumerate() {
            let key = format!("button-{index}");
            notification = notification.action(key.clone(), button.label);
            action_requests.insert(key, button.action_request);
        }
        Announced {
            tag: self.tag,
            notification,
            action_requests,
        }
    }
}
