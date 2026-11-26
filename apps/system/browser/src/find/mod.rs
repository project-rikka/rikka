mod bar;

pub(crate) use bar::bar;

use browser_preferences::{FindOption, FindOptions};
use rikka_app::prelude::*;
use rikka_ui::search::{Action, Step};
use rikka_web::{DisplayMatches, FindResult, Session};

use crate::shell::Shell;
use browser_model::TabId;
use browser_pages::Internal;

const SEEKING: &str = "rikka-browser-find";

#[derive(Debug, Default)]
pub(crate) struct Find {
    open: bool,
    field: TextDocument,
    query: String,
    matches: Option<u32>,
    current: u32,
    open_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FindStep {
    Query(String),
    Next,
    Previous,
    Finish,
}

impl Find {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.field
    }

    pub(crate) fn open_count(&self) -> u64 {
        self.open_count
    }

    pub(crate) fn open(&mut self, seed: Option<&str>) -> Option<FindStep> {
        self.open = true;
        self.open_count += 1;
        if let Some(seed) = seed.filter(|seed| !seed.is_empty() && !seed.contains('\n')) {
            self.field.set_text(seed);
        }
        self.field.select_all();
        self.typed()
    }

    pub(crate) fn typed(&mut self) -> Option<FindStep> {
        let text = self.field.text();
        if text == self.query {
            return None;
        }
        self.query = text.clone();
        self.matches = None;
        self.current = 0;
        Some(match text.is_empty() {
            true => FindStep::Finish,
            false => FindStep::Query(text),
        })
    }

    pub(crate) fn requery(&mut self) -> Option<FindStep> {
        self.query.clear();
        self.typed()
    }

    pub(crate) fn found(&mut self, result: &FindResult) -> bool {
        if !self.open || result.search_string != self.query {
            return false;
        }
        let matches = u32::try_from(result.total.max(0)).unwrap_or(0);
        let current = match result.found {
            true => u32::try_from(result.current.max(0))
                .unwrap_or(0)
                .min(matches),
            false => 0,
        };
        if self.matches == Some(matches) && self.current == current {
            return false;
        }
        self.matches = Some(matches);
        self.current = current;
        true
    }

    pub(crate) fn step(&mut self, forward: bool) -> Option<FindStep> {
        if !self.open || self.query.is_empty() {
            return None;
        }
        Some(match forward {
            true => FindStep::Next,
            false => FindStep::Previous,
        })
    }

    pub(crate) fn close(&mut self) -> Option<FindStep> {
        if !self.open {
            return None;
        }
        self.open = false;
        self.query.clear();
        self.matches = None;
        self.current = 0;
        Some(FindStep::Finish)
    }

    pub(crate) fn tally(&self) -> Option<(u32, u32)> {
        self.matches.map(|matches| (self.current, matches))
    }
}

fn find_request(seek: &FindStep, options: FindOptions) -> Option<rikka_web::Find> {
    let find = match seek {
        FindStep::Query(text) => rikka_web::Find::text(text.clone()),
        FindStep::Next => rikka_web::Find::next(),
        FindStep::Previous => rikka_web::Find::previous(),
        FindStep::Finish => return None,
    };
    Some(options.apply(find))
}

impl Shell {
    fn sought(&mut self, tab: TabId, view: Session, seek: FindStep) {
        let options = self.browser().preferences().find_options();
        let Some(find) = find_request(&seek, options) else {
            view.clear_matches();
            cancel((SEEKING, tab));
            return;
        };
        if matches!(seek, FindStep::Query(_)) {
            view.show_matches(DisplayMatches {
                highlight_all: true,
                dim_page: false,
                draw_outline: true,
            });
        }
        spawn_keyed(
            (SEEKING, tab),
            async move { view.find(find).await },
            move |shell: &mut Shell, result: rikka_web::Result<FindResult>| match result {
                Ok(result) => {
                    let page_services = shell.browser().page_services();
                    let mut requests = page_services.requests_mut();
                    if let Some(tab_requests) = requests.quietly(tab)
                        && tab_requests.find.found(&result)
                    {
                        requests.edit(tab);
                    }
                }
                Err(error) => tracing::debug!(%error, "find in page failed"),
            },
        );
    }

    fn finding(&mut self, tab: TabId, act: impl FnOnce(&mut Find) -> Option<FindStep>) {
        let seek = {
            let page_services = self.browser().page_services();
            let mut requests = page_services.requests_mut();
            act(&mut requests.edit(tab).find)
        };
        if let (Some(seek), Some(view)) = (seek, self.view_of(tab)) {
            self.sought(tab, view, seek);
        }
    }

    fn findable(&self) -> Option<TabId> {
        let model = self.model();
        let tab_entry = model.active(self.id())?;
        (tab_entry.internal.is_none() && tab_entry.page().view().is_some()).then_some(tab_entry.id)
    }

    pub(crate) fn find_command(&mut self, step: Option<bool>) {
        let source = self
            .model()
            .active(self.id())
            .is_some_and(|tab_entry| matches!(&*tab_entry.internal, Some(Internal::Source(_))));
        if source {
            let search = self.pages().source.search().clone();
            match step {
                None => search.open(None),
                Some(true) => search.act(Action::Step(Step::Next)),
                Some(false) => search.act(Action::Step(Step::Previous)),
            }
            return;
        }
        match step {
            None => self.open_find(),
            Some(forward) => self.find_again(forward),
        }
    }

    pub(crate) fn open_find(&mut self) {
        if let Some(tab) = self.findable() {
            self.finding(tab, |find| find.open(None));
        }
    }

    pub(crate) fn find_again(&mut self, forward: bool) {
        let Some(tab) = self.findable() else {
            return;
        };
        let open = self
            .browser()
            .page_services()
            .requests()
            .get(tab)
            .is_some_and(|tab_entry| tab_entry.find.is_open());
        match open {
            true => self.find_step(tab, forward),
            false => self.open_find(),
        }
    }

    pub(crate) fn find_typed(&mut self, tab: TabId) {
        self.finding(tab, Find::typed);
    }

    pub(crate) fn find_step(&mut self, tab: TabId, forward: bool) {
        self.finding(tab, |find| find.step(forward));
    }

    pub(crate) fn toggle_find_option(&mut self, tab: TabId, option: FindOption) {
        let preferences = self.browser().preferences();
        let on = !preferences.find_options().get(option);
        preferences.set_find_option(option, on);
        self.finding(tab, Find::requery);
    }

    pub(crate) fn close_find(&mut self, tab: TabId) {
        self.finding(tab, Find::close);
    }
}
