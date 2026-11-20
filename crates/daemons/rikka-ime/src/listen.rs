use std::sync::{Arc, Weak};

use rikka_bus::{Rule, Signals, Watch};
use rikka_types::Rect;
use zbus::blocking::Connection;
use zbus::message::Message;

use crate::error::{Error, Result};
use crate::inbox::Inbox;
use crate::names::{self, BUS};
use crate::state::State;

const ANNOUNCING: &str = "rikka-ime-panel-created";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Handled,
    FcitxAppeared,
}

pub(crate) fn start(panel: &Connection, inbox: &Arc<Inbox>) -> Result<Watch> {
    let inbox: Weak<Inbox> = Arc::downgrade(inbox);
    let announcer = panel.clone();
    Signals::new()
        .rule(
            Rule::new()
                .sender(names::FCITX)
                .path(names::INPUT_METHOD_PATH)
                .interface(names::INPUT_METHOD),
        )
        .rule(names::owner_changed())
        .each(
            panel,
            |message| Some(message.clone()),
            move |message| {
                let Some(inbox) = inbox.upgrade() else {
                    return false;
                };
                if dispatch(&message, &inbox) == Outcome::FcitxAppeared {
                    announce_panel(&announcer);
                }
                true
            },
        )
        .map_err(Error::bus)
}

pub(crate) fn announce_panel(panel: &Connection) {
    let announcing = panel.inner().clone();
    panel
        .inner()
        .executor()
        .spawn(
            async move {
                for (interface, created) in [
                    (names::PANEL_V1, "PanelCreated"),
                    (names::PANEL_V2, "PanelCreated2"),
                ] {
                    let sent = announcing
                        .emit_signal(None::<&str>, names::PANEL_PATH, interface, created, &())
                        .await;
                    if let Err(error) = sent {
                        tracing::warn!("failed to emit {interface}.{created}: {error}");
                    }
                }
            },
            ANNOUNCING,
        )
        .detach();
}

fn dispatch(message: &Message, inbox: &Inbox) -> Outcome {
    let header = message.header();
    let Some(member) = header.member() else {
        return Outcome::Handled;
    };
    if header.interface().is_some_and(|name| name == BUS) {
        return owner(message, inbox);
    }
    let body = message.body();
    match member.as_str() {
        "UpdatePreeditText" => {
            if let Ok((text, attributes)) = body.deserialize::<(String, String)>() {
                inbox.update(|state| state.set_preedit(text, attributes));
            }
        }
        "UpdatePreeditCaret" => {
            if let Ok(caret) = body.deserialize::<i32>() {
                inbox.update(|state| state.set_caret(caret));
            }
        }
        "ShowPreedit" => {
            if let Ok(shown) = body.deserialize::<bool>() {
                inbox.update(|state| state.show_preedit(shown));
            }
        }
        "UpdateAux" => {
            if let Ok((text, _)) = body.deserialize::<(String, String)>() {
                inbox.update(|state| state.set_aux(text));
            }
        }
        "ShowAux" => {
            if let Ok(shown) = body.deserialize::<bool>() {
                inbox.update(|state| state.show_aux(shown));
            }
        }
        "UpdateLookupTableCursor" => {
            if let Ok(cursor) = body.deserialize::<i32>() {
                inbox.update(|state| state.set_focus(cursor));
            }
        }
        "ShowLookupTable" => {
            if let Ok(shown) = body.deserialize::<bool>() {
                inbox.update(|state| state.show_candidates(shown));
            }
        }
        "UpdateSpotLocation" => {
            if let Ok((x, y)) = body.deserialize::<(i32, i32)>() {
                let spot = Rect::new(x as f32, y as f32, 0.0, 0.0);
                inbox.update(|state| state.set_spot(spot));
            }
        }
        "UpdateProperty" => {
            if let Ok(property) = body.deserialize::<String>() {
                inbox.update(|state| state.update_property(&property));
            }
        }
        "RegisterProperties" => {
            if let Ok(properties) = body.deserialize::<Vec<String>>() {
                inbox.update(|state| state.register_properties(&properties));
            }
        }
        "RemoveProperty" => {
            if let Ok(key) = body.deserialize::<String>() {
                inbox.update(|state| state.remove_property(&key));
            }
        }
        "ExecMenu" => {
            if let Ok(entries) = body.deserialize::<Vec<String>>() {
                inbox.update(|state| state.exec_menu(&entries));
            }
        }
        "Enable" => {
            if let Ok(enabled) = body.deserialize::<bool>() {
                inbox.update(|state| state.enable(enabled));
            }
        }
        _ => {}
    }
    Outcome::Handled
}

fn owner(message: &Message, inbox: &Inbox) -> Outcome {
    let Ok((_, _, new)) = message.body().deserialize::<(String, String, String)>() else {
        return Outcome::Handled;
    };
    inbox.update(State::forget);
    match new.is_empty() {
        true => Outcome::Handled,
        false => Outcome::FcitxAppeared,
    }
}
