use std::time::Duration;

use crate::event::Id;
use crate::picture::Icon;

pub const DEFAULT_ACTION: &str = "default";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    pub(crate) fn level(self) -> u8 {
        match self {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Timeout {
    #[default]
    Default,
    Never,
    After(Duration),
}

impl Timeout {
    pub(crate) fn milliseconds(self) -> i32 {
        match self {
            Timeout::Default => -1,
            Timeout::Never => 0,
            Timeout::After(after) => i32::try_from(after.as_millis().max(1)).unwrap_or(i32::MAX),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notification {
    pub(crate) summary: String,
    pub(crate) body: String,
    pub(crate) icon: Option<Icon>,
    pub(crate) app_name: String,
    pub(crate) desktop_entry: Option<String>,
    pub(crate) urgency: Urgency,
    pub(crate) category: Option<String>,
    pub(crate) actions: Vec<Action>,
    pub(crate) timeout: Timeout,
    pub(crate) replaces: Option<Id>,
    pub(crate) transient: bool,
    pub(crate) resident: bool,
}

impl Notification {
    pub fn new(summary: impl Into<String>) -> Notification {
        Notification {
            summary: summary.into(),
            ..Notification::default()
        }
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<String>) -> Notification {
        self.body = body.into();
        self
    }

    #[must_use]
    pub fn icon(mut self, icon: impl Into<Icon>) -> Notification {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn app_name(mut self, name: impl Into<String>) -> Notification {
        self.app_name = name.into();
        self
    }

    #[must_use]
    pub fn desktop_entry(mut self, entry: impl Into<String>) -> Notification {
        self.desktop_entry = Some(entry.into());
        self
    }

    #[must_use]
    pub fn urgency(mut self, urgency: Urgency) -> Notification {
        self.urgency = urgency;
        self
    }

    #[must_use]
    pub fn category(mut self, category: impl Into<String>) -> Notification {
        self.category = Some(category.into());
        self
    }

    #[must_use]
    pub fn action(mut self, key: impl Into<String>, label: impl Into<String>) -> Notification {
        let key = key.into();
        self.actions.retain(|action| action.key != key);
        self.actions.push(Action {
            key,
            label: label.into(),
        });
        self
    }

    #[must_use]
    pub fn default_action(self, label: impl Into<String>) -> Notification {
        self.action(DEFAULT_ACTION, label)
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Notification {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn replaces(mut self, id: Id) -> Notification {
        self.replaces = Some(id);
        self
    }

    #[must_use]
    pub fn transient(mut self, transient: bool) -> Notification {
        self.transient = transient;
        self
    }

    #[must_use]
    pub fn resident(mut self, resident: bool) -> Notification {
        self.resident = resident;
        self
    }

    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
}
