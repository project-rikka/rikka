use std::fmt;

use crate::identifier::LabelId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SystemLabel {
    Inbox,
    AllDrafts,
    AllSent,
    Trash,
    Spam,
    AllMail,
    Archive,
    Sent,
    Drafts,
    Outbox,
    Starred,
    AllScheduled,
}

const ALL: [SystemLabel; 12] = [
    SystemLabel::Inbox,
    SystemLabel::AllDrafts,
    SystemLabel::AllSent,
    SystemLabel::Trash,
    SystemLabel::Spam,
    SystemLabel::AllMail,
    SystemLabel::Archive,
    SystemLabel::Sent,
    SystemLabel::Drafts,
    SystemLabel::Outbox,
    SystemLabel::Starred,
    SystemLabel::AllScheduled,
];

impl SystemLabel {
    #[must_use]
    pub fn all() -> &'static [SystemLabel] {
        &ALL
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            SystemLabel::Inbox => "0",
            SystemLabel::AllDrafts => "1",
            SystemLabel::AllSent => "2",
            SystemLabel::Trash => "3",
            SystemLabel::Spam => "4",
            SystemLabel::AllMail => "5",
            SystemLabel::Archive => "6",
            SystemLabel::Sent => "7",
            SystemLabel::Drafts => "8",
            SystemLabel::Outbox => "9",
            SystemLabel::Starred => "10",
            SystemLabel::AllScheduled => "12",
        }
    }

    #[must_use]
    pub fn id(self) -> LabelId {
        LabelId::from(self.as_str())
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<SystemLabel> {
        ALL.into_iter().find(|label| label.as_str() == id)
    }
}

impl fmt::Display for SystemLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<SystemLabel> for LabelId {
    fn from(label: SystemLabel) -> LabelId {
        label.id()
    }
}

impl LabelId {
    #[must_use]
    pub fn as_system(&self) -> Option<SystemLabel> {
        SystemLabel::from_id(self.as_str())
    }

    #[must_use]
    pub fn is_system(&self) -> bool {
        self.as_system().is_some()
    }
}
