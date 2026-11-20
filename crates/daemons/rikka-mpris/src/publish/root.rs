use futures_channel::mpsc::UnboundedSender;
use zbus::interface;

use crate::publish::playback::Command;

pub(crate) struct Root {
    pub(crate) identity: String,
    pub(crate) desktop_entry: Option<String>,
    pub(crate) raises: bool,
    pub(crate) quits: bool,
    pub(crate) commands: UnboundedSender<Command>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {
        if self.raises {
            let _ = self.commands.unbounded_send(Command::Raise);
        }
    }

    fn quit(&self) {
        if self.quits {
            let _ = self.commands.unbounded_send(Command::Quit);
        }
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        self.quits
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        self.raises
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> String {
        self.identity.clone()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn desktop_entry(&self) -> String {
        self.desktop_entry.clone().unwrap_or_default()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}
