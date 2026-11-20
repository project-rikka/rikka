use rikka_app::prelude::*;
use rikka_nix::{Lines, Monitor, Page, RunId, Tail};
use rikka_ui::search::{Action, Search};

const LOADING: &str = "rikka-updates-transcript";

#[derive(State)]
pub(crate) struct Log {
    run: Option<RunId>,
    document: TextDocument,
    search: Search,
    place: Place,
    tail: Tail,
    loading: bool,
    behind: bool,
    failure: Option<String>,
}

impl Log {
    pub(crate) fn new() -> Self {
        Self {
            run: None,
            document: TextDocument::new(""),
            search: Search::new(),
            place: Place::new(),
            tail: Tail::new(),
            loading: false,
            behind: false,
            failure: None,
        }
    }

    pub(crate) fn document(&self) -> &TextDocument {
        &self.document
    }

    pub(crate) fn search(&self) -> &Search {
        &self.search
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn text(&self) -> String {
        self.document.text()
    }

    pub(crate) fn open_find(&mut self) {
        let selection = self.document.selected_text();
        self.search
            .open(Some(selection.as_str()).filter(|selection| !selection.is_empty()));
    }

    pub(crate) fn find(&mut self, action: Action) {
        self.search.act(action);
    }

    pub(crate) fn show(&mut self, run: RunId, monitor: &Monitor) {
        if self.run == Some(run) {
            return;
        }
        self.run = Some(run);
        self.document.reset("");
        self.tail = Tail::new();
        self.behind = false;
        self.failure = None;
        self.fetch(monitor.clone(), 0);
    }

    pub(crate) fn receive(&mut self, lines: Lines, monitor: &Monitor) {
        if self.run != Some(lines.run) {
            return;
        }
        match self.tail.accept(lines.from, &lines.lines) {
            Ok(fresh) => self.append(fresh),
            Err(_) if self.loading => self.behind = true,
            Err(_) => self.fetch(monitor.clone(), self.tail.next()),
        }
    }

    fn fetch(&mut self, monitor: Monitor, from: u64) {
        let Some(run) = self.run else {
            return;
        };
        self.loading = true;
        spawn_blocking_keyed(
            LOADING,
            move || {
                let page = monitor.transcript(run, from);
                (page, monitor)
            },
            |log: &mut Log, (page, monitor): (rikka_nix::Result<Page>, Monitor)| {
                log.landed(page, monitor);
            },
        );
    }

    fn landed(&mut self, page: rikka_nix::Result<Page>, monitor: Monitor) {
        let page = match page {
            Ok(page) if Some(page.run) == self.run => page,
            Ok(_) => return,
            Err(error) => {
                self.loading = false;
                self.failure = Some(error.to_string());
                return;
            }
        };
        match self.tail.accept(page.from, &page.lines) {
            Ok(fresh) => self.append(fresh),
            Err(_) => return self.fetch(monitor, self.tail.next()),
        }
        if !page.end || std::mem::take(&mut self.behind) {
            return self.fetch(monitor, self.tail.next());
        }
        self.loading = false;
    }

    fn append(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let mut text = String::with_capacity(lines.iter().map(|line| line.len() + 1).sum());
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        let following = !self.place.is_scrolled_away();
        self.document.append(text);
        if following {
            self.place.end();
        }
    }
}

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Log")
            .field("run", &self.run)
            .field("next", &self.tail.next())
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}
