use std::sync::Arc;

use code_meaning::Meaning;
use rikka_clipboard::{Clipboard, Offer, Selection};
use rikka_seam::SeamClient;
use rikka_session::settings_app::Section;
use rikka_ui::tr;
use serde::{Deserialize, Serialize};
use shell_kit::detached;
use shell_kit::launch::Spawn;
use shell_notifications::Announcement;

use crate::error::{Error, Result};
use crate::scan::codes::Code;

const SHOWN: usize = 120;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Act {
    Open,
    Copy,
    Join,
    Settings,
}

impl Act {
    #[must_use]
    pub(crate) fn primary(meaning: &Meaning) -> Act {
        match meaning {
            Meaning::Link(_) => Act::Open,
            Meaning::Network(invitation) if invitation.security.needs_account() => Act::Settings,
            Meaning::Network(_) => Act::Join,
            Meaning::Text => Act::Copy,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct Acting {
    pub(crate) code: usize,
    pub(crate) act: Act,
}

struct Strings {
    app_name: String,
    copied: String,
    failed: String,
}

impl Strings {
    fn now() -> Strings {
        Strings {
            app_name: tr!("shell.capture.scan.name").to_string(),
            copied: tr!("shell.capture.scan.copied").to_string(),
            failed: tr!("shell.capture.scan.not-copied").to_string(),
        }
    }
}

pub(crate) fn open(address: &str, seam: &SeamClient) {
    seam.notify(Spawn::new(rikka_open::opener(address)).request());
}

pub(crate) fn settings(seam: &SeamClient) {
    seam.notify(Spawn::settings(Section::Network).request());
}

pub(crate) fn copy(code: &Code, clipboard: Option<Arc<Clipboard>>, seam: SeamClient) {
    let copying = CopyJob::from_code(code);
    let words = Strings::now();
    detached("rikka-capture-copy", move || {
        match copying.publish(clipboard.as_deref()) {
            Ok(()) => {
                let announcement = Announcement::new(&words.app_name, &words.copied);
                match copying.secret {
                    true => announcement,
                    false => announcement.body(shortened(&copying.text)),
                }
                .send(&seam);
            }
            Err(error) => {
                tracing::warn!("failed to copy code: {error}");
                Announcement::new(&words.app_name, &words.failed)
                    .body(error.to_string())
                    .urgent()
                    .send(&seam);
            }
        }
    });
}

struct CopyJob {
    text: String,
    secret: bool,
}

impl CopyJob {
    fn from_code(code: &Code) -> CopyJob {
        match &code.meaning {
            Meaning::Link(link) => CopyJob {
                text: link.address().to_owned(),
                secret: false,
            },
            Meaning::Network(invitation) if !invitation.passphrase.is_empty() => CopyJob {
                text: invitation.passphrase.clone(),
                secret: true,
            },
            Meaning::Network(invitation) => CopyJob {
                text: invitation.ssid.name().into_owned(),
                secret: false,
            },
            Meaning::Text => CopyJob {
                text: code.text.clone(),
                secret: false,
            },
        }
    }

    fn publish(&self, clipboard: Option<&Clipboard>) -> Result<()> {
        let clipboard = clipboard.ok_or(Error::NoClipboard)?;
        let offer = Offer::text(&self.text);
        let offer = match self.secret {
            true => offer.secret(),
            false => offer,
        };
        Ok(clipboard.publish(Selection::Clipboard, offer)?)
    }
}

fn shortened(text: &str) -> String {
    match text.char_indices().nth(SHOWN) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}
