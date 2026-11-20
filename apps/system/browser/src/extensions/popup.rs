use rikka_app::prelude::*;
use rikka_web::{ActionKind, Event, Session, SessionSettings, web_view};

use crate::extensions::actions::action_key;
use crate::requests::{DropdownOwner, DropdownSlot, dropdown_view};
use crate::shell::Shell;
use browser_model::TabId;

const WIRE: &str = "rikka-browser-extension-popup";
const KEY: &str = "rikka-browser-extension-popup-panel";
const WIDTH: f32 = 380.0;
const HEIGHT: f32 = 520.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenPopup {
    tab: TabId,
    extension: String,
    kind: ActionKind,
    view: Session,
}

#[derive(State)]
pub(crate) struct Popup {
    shown: Tracked<Option<OpenPopup>>,
    pub(crate) dropdown: Tracked<DropdownSlot>,
}

impl Popup {
    pub(crate) fn new() -> Popup {
        Popup {
            shown: Tracked::new(None),
            dropdown: Tracked::default(),
        }
    }
}

impl std::fmt::Debug for Popup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Popup")
            .field("shown", &self.shown.as_ref().map(|shown| &shown.extension))
            .finish()
    }
}

impl Shell {
    pub(crate) fn open_action_popup(
        &mut self,
        tab: TabId,
        extension: String,
        kind: ActionKind,
        uri: String,
    ) {
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            return;
        };
        let settings = SessionSettings::new()
            .extension_popup(true)
            .handle_load_requests(true);
        let session = match self
            .browser()
            .engine(key)
            .and_then(|runtime| runtime.session(settings))
        {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!(%error, "failed to open extension popup");
                return;
            }
        };
        self.close_action_popup();
        let events = session.events();
        session.load(uri);
        let view = session;
        if let Some(events) = events {
            stream_keyed(WIRE, events, move |shell: &mut Shell, event: Event| {
                shell.handle_popup_event(tab, event)
            });
        }
        self.popup.shown.set(Some(OpenPopup {
            tab,
            extension,
            kind,
            view,
        }));
    }

    fn handle_popup_event(&mut self, tab: TabId, event: Event) {
        match event {
            Event::CloseRequested | Event::Closed(_) => self.close_action_popup(),
            Event::LoadRequest(request) => request.allow(),
            Event::NewSession(request) => {
                let address = request.uri.clone();
                match request.open() {
                    Ok(session) => {
                        self.close_action_popup();
                        self.adopt_page(crate::page::Live::adopt(session), address, true);
                    }
                    Err(error) => tracing::warn!(%error, "failed to open popup window"),
                }
            }
            Event::Prompt(prompt) => self.prompted(tab, prompt),
            Event::Dropdown(dropdown) => self.dropdown_opened(DropdownOwner::ActionPopup, dropdown),
            Event::DropdownUpdated(update) => {
                self.dropdown_updated(DropdownOwner::ActionPopup, update)
            }
            Event::DropdownHidden(id) => self.dropdown_hidden(DropdownOwner::ActionPopup, id),
            unheard => drop(unheard),
        }
    }

    pub(crate) fn close_action_popup(&mut self) {
        cancel(WIRE);
        let dropdown = std::mem::take(&mut *self.popup.dropdown);
        drop(dropdown);
        if let Some(shown) = self.popup.shown.take() {
            shown.view.close();
        }
    }
}

pub(crate) fn popup(shell: &Shell) -> Option<Element> {
    let shown = shell.popup.shown.as_ref()?;
    let dropdown = &*shell.popup.dropdown;
    let page = web_view(&shown.view)
        .retain_focus(dropdown.is_open())
        .size(Length::px(WIDTH), Length::px(HEIGHT));
    let content = column_of(
        std::iter::once(page.any()).chain(dropdown_view(dropdown, DropdownOwner::ActionPopup)),
    );
    let card = container(content).radius(Corner::Panel).clipped();
    let anchor = action_key(&shown.extension, shown.kind);
    Some(
        popover(card)
            .flush()
            .on_dismiss(|shell: &mut Shell| shell.close_action_popup())
            .build()
            .keyed((KEY, &anchor))
            .anchored(Anchor::new(anchor, AnchorEdge::BelowEnd).offset(0.0, 6.0)),
    )
}
