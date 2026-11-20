use serde::{Deserialize, Serialize};

use crate::identity::User;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ask {
    pub id: u64,
    pub action: String,
    pub message: String,
    pub details: Vec<(String, String)>,
    pub users: Vec<User>,
    pub chosen: usize,
    pub question: Option<Question>,
    pub notice: Option<Notice>,
    pub failures: u32,
}

impl Ask {
    #[must_use]
    pub fn user(&self) -> Option<&User> {
        self.users.get(self.chosen)
    }

    #[must_use]
    pub fn waiting(&self) -> bool {
        self.question.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Question {
    pub serial: u64,
    pub text: String,
    pub echo: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
}

impl Notice {
    #[must_use]
    pub(crate) fn info(text: String) -> Notice {
        Notice {
            kind: NoticeKind::Info,
            text,
        }
    }

    #[must_use]
    pub(crate) fn error(text: String) -> Notice {
        Notice {
            kind: NoticeKind::Error,
            text,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NoticeKind {
    Info,
    Error,
}
