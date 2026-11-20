use std::collections::HashMap;
use std::time::SystemTime;

use zbus::zvariant::OwnedValue;

use crate::action::{self, Action};
use crate::body::{self, Body};
use crate::hints::Hints;
use crate::image::Image;
use crate::markup;
use crate::progress::Progress;
use crate::timeout::Timeout;
use crate::urgency::Urgency;

const MAX_SUMMARY: usize = 160;

const MAX_APP_NAME: usize = 64;

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: Body,
    pub actions: Vec<Action>,
    pub image: Option<Image>,
    pub urgency: Urgency,
    pub desktop_entry: Option<String>,
    pub timeout: Timeout,
    pub resident: bool,
    pub action_icons: bool,
    pub transient: bool,
    pub progress: Option<Progress>,
    pub arrived: SystemTime,
}

#[derive(Debug)]
pub(crate) struct Posting {
    pub(crate) app_name: String,
    pub(crate) app_icon: String,
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) actions: Vec<String>,
    pub(crate) hints: HashMap<String, OwnedValue>,
    pub(crate) expire_timeout: i32,
}

impl Notification {
    #[must_use]
    pub fn new(app_name: impl Into<String>, summary: impl Into<String>) -> Notification {
        Notification {
            id: 0,
            app_name: line(&app_name.into(), MAX_APP_NAME),
            summary: line(&summary.into(), MAX_SUMMARY),
            body: Body::default(),
            actions: Vec::new(),
            image: None,
            urgency: Urgency::default(),
            desktop_entry: None,
            timeout: Timeout::Default,
            resident: false,
            action_icons: false,
            transient: false,
            progress: None,
            arrived: SystemTime::now(),
        }
    }

    #[must_use]
    pub fn body(mut self, text: impl Into<String>) -> Notification {
        self.body = body::capped(Body {
            text: text
                .into()
                .chars()
                .filter(|character| body::drawable(*character))
                .collect(),
            ..Body::default()
        });
        self
    }

    #[must_use]
    pub fn image(mut self, image: Image) -> Notification {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn action(mut self, key: impl Into<String>, label: impl Into<String>) -> Notification {
        self.actions.push(Action {
            key: key.into(),
            label: label.into(),
        });
        self
    }

    #[must_use]
    pub fn urgency(mut self, urgency: Urgency) -> Notification {
        self.urgency = urgency;
        self
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Notification {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn resident(mut self) -> Notification {
        self.resident = true;
        self
    }

    #[must_use]
    pub fn progress(mut self, progress: Progress) -> Notification {
        self.progress = Some(progress);
        self
    }

    #[must_use]
    pub(crate) fn read(posting: Posting) -> Notification {
        let hints = Hints::read(&posting.hints);
        Notification {
            id: 0,
            app_name: line(&posting.app_name, MAX_APP_NAME),
            summary: line(&posting.summary, MAX_SUMMARY),
            body: markup::read(&posting.body),
            actions: action::read(posting.actions),
            image: hints.image.or_else(|| Image::named(&posting.app_icon)),
            urgency: hints.urgency,
            desktop_entry: hints.desktop_entry,
            timeout: Timeout::from_millis(posting.expire_timeout),
            resident: hints.resident,
            action_icons: hints.action_icons,
            transient: hints.transient,
            progress: hints.progress,
            arrived: SystemTime::now(),
        }
    }

    #[must_use]
    pub fn default_action(&self) -> Option<&Action> {
        self.actions.iter().find(|action| action.is_default())
    }

    pub fn buttons(&self) -> impl Iterator<Item = &Action> {
        self.actions.iter().filter(|action| !action.is_default())
    }
}

fn line(text: &str, most: usize) -> String {
    let mut line = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if line.chars().count() >= most {
            break;
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(word.chars().filter(|character| body::drawable(*character)));
    }
    if let Some((at, _)) = line.char_indices().nth(most) {
        line.truncate(at);
        line.push('…');
    }
    line
}
