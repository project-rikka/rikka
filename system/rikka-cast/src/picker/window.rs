use rikka_app::Sender;
use rikka_portal::Reply;
use rikka_ui::{State, spawn_blocking};

use crate::picker::APP_ID;
use crate::picker::sources::{self, Listing, Preview, SourceEntry};
use crate::protocol::{Answer, CastSource, Outcome, PickerRequest, SourceKind};

#[derive(State)]
pub(crate) struct Picker {
    request: PickerRequest,
    kinds: Vec<SourceKind>,
    tab: usize,
    monitors: Vec<SourceEntry>,
    windows: Vec<SourceEntry>,
    selected: Vec<CastSource>,
    remember: bool,
    loading: bool,
    failure: Option<String>,
    sender: Sender<Picker>,
    reply: Reply,
}

impl Picker {
    pub(crate) fn new(sender: Sender<Picker>, request: PickerRequest, reply: Reply) -> Picker {
        let kinds: Vec<SourceKind> = [SourceKind::Monitor, SourceKind::Window]
            .into_iter()
            .filter(|kind| request.offers(*kind))
            .collect();
        Picker {
            request,
            kinds,
            tab: 0,
            monitors: Vec::new(),
            windows: Vec::new(),
            selected: Vec::new(),
            remember: false,
            loading: true,
            failure: None,
            sender,
            reply,
        }
    }

    pub(crate) fn request(&self) -> &PickerRequest {
        &self.request
    }

    pub(crate) fn kinds(&self) -> &[SourceKind] {
        &self.kinds
    }

    pub(crate) fn tab(&self) -> usize {
        self.tab
    }

    pub(crate) fn visible_sources(&self) -> &[SourceEntry] {
        match self.kinds.get(self.tab) {
            Some(SourceKind::Window) => &self.windows,
            _ => &self.monitors,
        }
    }

    pub(crate) fn shows_windows(&self) -> bool {
        matches!(self.kinds.get(self.tab), Some(SourceKind::Window))
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn is_selected(&self, source: &CastSource) -> bool {
        self.selected.contains(source)
    }

    pub(crate) fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    pub(crate) fn is_remembered(&self) -> bool {
        self.remember
    }

    pub(crate) fn load_sources(&mut self) {
        let kinds = self.kinds.clone();
        spawn_blocking(
            move || sources::list(&kinds, APP_ID),
            |picker: &mut Picker, listing: Listing| picker.apply_listing(listing),
        );
    }

    pub(crate) fn select_tab(&mut self, tab: usize) {
        self.tab = tab.min(self.kinds.len().saturating_sub(1));
    }

    pub(crate) fn toggle(&mut self, source: CastSource) {
        if let Some(at) = self
            .selected
            .iter()
            .position(|selected| *selected == source)
        {
            self.selected.remove(at);
            return;
        }
        if !self.request.multiple {
            self.selected.clear();
        }
        self.selected.push(source);
    }

    pub(crate) fn remember(&mut self, remember: bool) {
        self.remember = remember;
    }

    pub(crate) fn share(&self) {
        if !self.has_selection() {
            return;
        }
        self.reply.answer(&Outcome::Chosen(Answer {
            sources: self.selected.clone(),
            remember: self.remember && self.request.persist.is_offered(),
        }))
    }

    pub(crate) fn cancel(&self) {
        self.reply.answer(&Outcome::Cancelled)
    }

    fn apply_listing(&mut self, listing: Listing) {
        self.loading = false;
        self.failure = listing.failure;
        self.monitors = listing.monitors;
        self.windows = listing.windows;

        let sources: Vec<CastSource> = self
            .monitors
            .iter()
            .chain(self.windows.iter())
            .map(|entry| entry.source.clone())
            .collect();
        let sender = self.sender.clone();
        let previews = std::thread::Builder::new()
            .name("rikka-cast-preview".to_owned())
            .spawn(move || {
                for source in sources {
                    let preview = sources::preview(&source);
                    if sender
                        .send(move |picker: &mut Picker| picker.set_preview(source, preview))
                        .is_err()
                    {
                        return;
                    }
                }
            });
        if let Err(error) = previews {
            eprintln!("rikka-cast-picker: cannot load previews: {error}");
        }
    }

    fn set_preview(&mut self, source: CastSource, preview: Option<Preview>) {
        let matching = self
            .monitors
            .iter_mut()
            .chain(self.windows.iter_mut())
            .find(|entry| entry.source == source);
        if let Some(entry) = matching {
            entry.preview = preview;
        }
    }
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picker")
            .field("kinds", &self.kinds)
            .field("chosen", &self.selected)
            .finish_non_exhaustive()
    }
}
