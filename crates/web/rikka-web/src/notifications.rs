use std::sync::Arc;

use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::rikka::notifications::{
    NotificationAction, NotificationClicked, NotificationClosed, ShowNotification,
};

use crate::wire::Link;

#[derive(Debug)]
pub struct Notification {
    pub name: String,
    pub title: String,
    pub text: String,
    pub image_uri: Option<String>,
    pub origin: Option<String>,
    pub language: Option<String>,
    pub direction: Option<String>,
    pub silent: bool,
    pub requires_interaction: bool,
    pub private: bool,
    pub actions: Vec<NotificationAction>,
    link: Option<Arc<Link>>,
}

impl Notification {
    pub(crate) fn from_wire(shown: ShowNotification, link: &Arc<Link>) -> Notification {
        Notification {
            name: shown.name,
            title: shown.title,
            text: shown.text,
            image_uri: shown.image_url,
            origin: shown.origin,
            language: shown.lang,
            direction: shown.dir,
            silent: shown.silent,
            requires_interaction: shown.require_interaction,
            private: shown.private,
            actions: shown.actions,
            link: Some(Arc::clone(link)),
        }
    }

    pub fn click(mut self, action: Option<&str>) {
        if let Some(link) = self.link.take() {
            link.send(
                Target::Runtime,
                &NotificationClicked {
                    name: self.name.clone(),
                    action: action.map(str::to_owned),
                },
            );
        }
    }

    pub fn close(self) {
        drop(self);
    }
}

impl Drop for Notification {
    fn drop(&mut self) {
        if let Some(link) = self.link.take() {
            link.send(
                Target::Runtime,
                &NotificationClosed {
                    name: self.name.clone(),
                },
            );
        }
    }
}
