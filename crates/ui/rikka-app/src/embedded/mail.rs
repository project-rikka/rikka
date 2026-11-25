use std::sync::mpsc::{self, Receiver, Sender};

use rikka_appearance::Appearance;
use rikka_theme::Theme;
use rikka_ui::RuntimeMessage;
use rikka_ui_host::TaskSink;

use crate::embedded::SurfaceId;

pub(crate) enum Mail {
    Message {
        surface: SurfaceId,
        inbound: RuntimeMessage,
    },
    Appearance {
        appearance: Box<Appearance>,
        theme: Option<Box<Theme>>,
    },
    #[cfg(unix)]
    Language,
}

#[derive(Clone)]
pub(crate) struct Post(Sender<Mail>);

impl Post {
    pub(crate) fn send(&self, mail: Mail) -> bool {
        self.0.send(mail).is_ok()
    }
}

impl TaskSink<SurfaceId> for Post {
    fn deliver(&self, surface: SurfaceId, inbound: RuntimeMessage) -> bool {
        self.send(Mail::Message { surface, inbound })
    }
}

pub(crate) struct Mailbox(Receiver<Mail>);

impl Mailbox {
    pub(crate) fn take(&self) -> Option<Mail> {
        self.0.try_recv().ok()
    }
}

pub(crate) fn mailbox() -> (Post, Mailbox) {
    let (sender, receiver) = mpsc::channel();
    (Post(sender), Mailbox(receiver))
}

impl std::fmt::Debug for Post {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Post").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Mailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mailbox").finish_non_exhaustive()
    }
}
