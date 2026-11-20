use files_core::Location;
use files_find::{Finished, Options, Query, Search, Sink};
use rikka_ui::TextDocument;

const BATCH: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct Finding {
    text: TextDocument,
    searched: u64,
    contents: bool,
    walk: Option<Search>,
    pending: Vec<Location>,
    running: bool,
}

impl Finding {
    pub(crate) fn text(&self) -> &TextDocument {
        &self.text
    }

    pub(crate) fn searches_contents(&self) -> bool {
        self.contents
    }

    pub(crate) fn toggle_contents(&mut self) {
        self.contents = !self.contents;
    }

    pub(crate) fn query_changed(&mut self) -> bool {
        let written = self.text.revision();
        let changed = self.searched != written;
        self.searched = written;
        changed
    }

    pub(crate) fn start(
        &mut self,
        at: &Location,
        show_hidden: bool,
        sink: Sink,
        over: Finished,
    ) -> bool {
        let typed = self.text.text();
        let query = Query {
            name: typed.clone(),
            contents: match self.contents {
                true => typed,
                false => String::new(),
            },
            options: Options {
                hidden: show_hidden,
                ..Options::default()
            },
        };
        self.stop();
        if query.is_empty() {
            return false;
        }
        self.running = true;
        self.walk = Some(files_find::start(at, &query, sink, over));
        true
    }

    pub(crate) fn found(&mut self, at: Location) -> bool {
        self.pending.push(at);
        self.pending.len() >= BATCH
    }

    pub(crate) fn take(&mut self) -> Vec<Location> {
        std::mem::take(&mut self.pending)
    }

    pub(crate) fn finished(&mut self) {
        self.running = false;
    }

    pub(crate) fn stop(&mut self) {
        self.walk = None;
        self.pending.clear();
        self.running = false;
    }

    pub(crate) fn clear(&mut self) {
        self.stop();
        self.text.set_text("");
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running
    }

    pub(crate) fn waiting(&self) -> usize {
        self.pending.len()
    }
}
