use rikka_sync::Mutex;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use rikka_app::Sender;
use rikka_app::prelude::Tracked;
use rikka_terminal::{Config, Report, Session};
use rikka_ui::MaterialSymbol;
use rikka_ui::search::{Action, Search};
use rikka_widgets::{Tab as Strip, tab};

use crate::opening::Program;
use crate::terminal::Terminal;

#[derive(Debug)]
struct Wire {
    sender: Sender<Terminal>,
    id: u64,
}

#[derive(Debug)]
struct Reporter(Mutex<Wire>);

impl Reporter {
    fn new(sender: Sender<Terminal>, id: u64) -> Self {
        Self(Mutex::new(Wire { sender, id }))
    }

    fn forward(&self, report: Report) {
        let wire = self.wire();
        let id = wire.id;
        let _ = wire
            .sender
            .send(move |terminal: &mut Terminal| terminal.report(id, report));
    }

    fn rewire(&self, sender: Sender<Terminal>, id: u64) {
        *self.wire() = Wire { sender, id };
    }

    fn wire(&self) -> rikka_sync::MutexGuard<'_, Wire> {
        self.0.lock()
    }
}

#[derive(Debug)]
pub(crate) struct Tab {
    id: u64,
    session: Rc<Session>,
    reporter: Arc<Reporter>,
    title: Tracked<Option<String>>,
    search: Search,
}
impl Tab {
    pub(crate) fn open(
        id: u64,
        config: &Config,
        sender: &Sender<Terminal>,
        at: Option<&Path>,
        program: Option<&Program>,
    ) -> rikka_terminal::Result<Self> {
        let options = config.options();
        let options = match at {
            Some(at) => options.working_directory(at),
            None => options,
        };
        let options = match program {
            Some(program) => options.shell(program.name.clone(), program.arguments.clone()),
            None => options,
        };
        let reporter = Arc::new(Reporter::new(sender.clone(), id));
        let forwarding = Arc::clone(&reporter);
        let session = Session::open(&options, move |report| forwarding.forward(report))?;

        Ok(Self {
            id,
            session: Rc::new(session),
            reporter,
            title: Tracked::new(None),
            search: Search::new(),
        })
    }

    pub(crate) fn reassigned(mut self, id: u64, sender: &Sender<Terminal>) -> Self {
        self.id = id;
        self.reporter.rewire(sender.clone(), id);
        self
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn session(&self) -> &Rc<Session> {
        &self.session
    }

    pub(crate) fn search(&self) -> &Search {
        &self.search
    }

    pub(crate) fn open_find(&self) {
        let selection = self.session.selected_text();
        self.search.open(selection.as_deref());
    }

    pub(crate) fn find(&self, action: Action) {
        self.search.act(action);
    }
    pub(crate) fn set_title(&mut self, title: Option<String>) -> bool {
        let title = title.filter(|title| !title.trim().is_empty());
        let retitled = *self.title != title;
        self.title.set(title);
        retitled
    }

    pub(crate) fn strip(&self, fallback: &str) -> Strip {
        let title = self.title.as_deref().unwrap_or(fallback);
        tab(title).icon(MaterialSymbol::TERMINAL)
    }
}
