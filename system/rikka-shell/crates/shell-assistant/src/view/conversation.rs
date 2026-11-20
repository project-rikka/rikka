use rikka_chat::{empty_state, transcript};
use rikka_ui::{Element, tr};

use crate::chat::ShellChat;
use crate::panel::Panel;
use crate::shown::Readiness;

pub(super) fn view(panel: &Panel) -> Element {
    transcript::<ShellChat, _>(panel.transcript(), || {
        empty_state(match panel.shown().readiness {
            Readiness::Opening => tr!("shell.assistant.opening"),
            Readiness::Unready => tr!("shell.assistant.unready"),
            Readiness::Ready => tr!("shell.assistant.start"),
        })
    })
}
