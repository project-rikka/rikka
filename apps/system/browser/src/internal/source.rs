use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use futures_util::StreamExt;
use rikka_app::prelude::*;
use rikka_syntax::Language;
use rikka_ui::search::{Action, Search};
use rikka_web::{Event, Session, World};
use rikka_widgets::{column, row};
use serde_json::Value;

use crate::browser::Browser;
use crate::internal::status::Status;
use crate::shell::Shell;
use browser_model::{Placement, ProfileKey, TabId};
use browser_pages::Internal;

const KEPT: usize = 8;
const FETCH: &str = "rikka-browser-source";
const FIND_MARK: &str = "rikka-browser-source-find";
const WORLD: &str = "rikka-source";
const READ: &str = "(async () => { const response = await fetch(document.URL, { cache: \"force-cache\", credentials: \"include\" }); return { uri: document.URL, mime: response.headers.get(\"content-type\"), text: await response.text() }; })()";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub(crate) page: String,
    pub(crate) text: String,
    pub(crate) language: Option<&'static str>,
}

fn decoded(page: String, read: &Value) -> Option<Source> {
    let text = read.get("text")?.as_str()?.to_owned();
    let uri = read.get("uri").and_then(Value::as_str).unwrap_or(&page);
    let mime = read.get("mime").and_then(Value::as_str);
    Some(Source {
        language: language_of(mime, uri),
        page,
        text,
    })
}

async fn read(view: Session, page: String) -> Result<Source, String> {
    let read = view
        .evaluate(READ, &World::isolated(WORLD))
        .await
        .map_err(|error| error.to_string())?;
    decoded(page, &read).ok_or_else(|| "malformed page source".to_owned())
}

fn language_of(mime: Option<&str>, uri: &str) -> Option<&'static str> {
    let essence = mime
        .and_then(|mime| mime.split(';').next())
        .map(|mime| mime.trim().to_ascii_lowercase());
    match essence.as_deref() {
        Some("text/html" | "application/xhtml+xml") => Some("html"),
        Some("text/css") => Some("css"),
        Some("text/javascript" | "application/javascript" | "application/x-javascript") => {
            Some("javascript")
        }
        Some("application/json") => Some("json"),
        Some("text/xml" | "application/xml" | "image/svg+xml" | "application/rss+xml") => {
            Some("xml")
        }
        _ if uri.ends_with(".js") => Some("javascript"),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub(crate) struct Sources {
    cached: VecDeque<(ProfileKey, Arc<Source>)>,
}

impl Sources {
    pub(crate) fn insert(&mut self, profile: ProfileKey, source: Arc<Source>) {
        self.cached
            .retain(|(candidate, cached)| *candidate != profile || cached.page != source.page);
        self.cached.push_front((profile, source));
        self.cached.truncate(KEPT);
    }

    pub(crate) fn get(&self, profile: ProfileKey, page: &str) -> Option<Arc<Source>> {
        self.cached
            .iter()
            .find(|(candidate, cached)| *candidate == profile && cached.page == page)
            .map(|(_, cached)| Arc::clone(cached))
    }

    pub(crate) fn forget(&mut self, profile: ProfileKey) {
        self.cached.retain(|(candidate, _)| *candidate != profile);
    }
}

#[derive(State)]
pub(crate) struct SourcePage {
    #[untracked]
    document: TextDocument,
    #[untracked]
    search: Search,
    #[untracked]
    generation: u64,
    page: Tracked<String>,
    language: Tracked<Option<&'static str>>,
    wrap: Tracked<bool>,
    status: Tracked<Status>,
}

impl SourcePage {
    pub(crate) fn new() -> SourcePage {
        SourcePage {
            document: TextDocument::new(""),
            search: Search::new(),
            generation: 0,
            page: Tracked::new(String::new()),
            language: Tracked::new(None),
            wrap: Tracked::new(false),
            status: Tracked::new(Status::Waiting),
        }
    }

    fn show(&mut self, source: &Source) {
        self.document.reset(source.text.clone());
        self.language.set(source.language);
        self.status.set(Status::Ready);
    }

    pub(crate) fn search(&self) -> &Search {
        &self.search
    }

    pub(crate) fn open(&mut self, browser: &Rc<Browser>, key: ProfileKey, page: String) {
        self.generation += 1;
        self.page.set(page.clone());
        self.search.close();
        self.status.set(Status::Waiting);
        self.document.reset(String::new());
        if let Some(source) = browser.page_services().sources().get(key, &page) {
            self.show(&source);
            return;
        }
        let view = browser
            .engine(key)
            .and_then(|runtime| runtime.session(crate::page::settings()));
        let view = match view {
            Ok(view) => view,
            Err(error) => {
                tracing::warn!(%error, "failed to read page source");
                self.status.set(Status::Failed);
                return;
            }
        };
        let (generation, browser) = (self.generation, Rc::clone(browser));
        spawn_keyed(
            FETCH,
            fetched(view, page),
            move |source_page: &mut SourcePage, read: Result<Source, String>| {
                if source_page.generation != generation {
                    return;
                }
                match read {
                    Ok(source) => {
                        let source = Arc::new(source);
                        browser
                            .page_services()
                            .sources_mut()
                            .insert(key, Arc::clone(&source));
                        source_page.show(&source);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to read page source");
                        source_page.status.set(Status::Failed);
                    }
                }
            },
        );
    }

    fn toggle_wrap(&mut self) {
        let wrap = !*self.wrap;
        self.wrap.set(wrap);
    }
}

impl std::fmt::Debug for SourcePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourcePage")
            .field("page", &*self.page)
            .field("status", &*self.status)
            .finish_non_exhaustive()
    }
}

async fn fetched(view: Session, page: String) -> Result<Source, String> {
    let mut events = view
        .events()
        .ok_or_else(|| "page events already taken".to_owned())?;
    view.set_visible(false);
    view.load(page.clone());
    while let Some(event) = events.next().await {
        match event {
            Event::LoadRequest(request) => request.allow(),
            Event::PageStopped { success: true } => break,
            Event::PageStopped { success: false } => {
                return Err("page load failed".to_owned());
            }
            Event::LoadError(failure) => {
                return Err(format!("page load failed: {:?}", failure.error));
            }
            _ => {}
        }
    }
    let source = read(view.clone(), page).await;
    view.close();
    source
}

impl Shell {
    pub(crate) fn view_active_source(&mut self) {
        let active = self.active_tab();
        if let Some(tab) = active {
            self.view_source(tab);
        }
    }

    pub(crate) fn view_source(&mut self, tab: TabId) {
        let Some((view, page, profile)) = self
            .model()
            .tab(tab)
            .filter(|candidate| candidate.internal.is_none())
            .and_then(|tab_entry| {
                Some((
                    tab_entry.page().view()?.clone(),
                    (*tab_entry.meta.url).clone(),
                    tab_entry.profile,
                ))
            })
        else {
            return;
        };
        if page.is_empty() {
            return;
        }
        spawn(
            read(view, page.clone()),
            move |shell: &mut Shell, read: Result<Source, String>| {
                match read {
                    Ok(source) => {
                        shell
                            .browser()
                            .page_services()
                            .sources_mut()
                            .insert(profile, Arc::new(source));
                    }
                    Err(error) => tracing::debug!(%error, "failed to read page source"),
                }
                let address = Internal::Source(page.clone()).address();
                shell.browser().edit().open_tab(
                    shell.id(),
                    Some(address),
                    Placement::After(tab),
                    true,
                );
                shell.changed();
            },
        );
    }
}

pub(crate) fn view(page: &SourcePage) -> Element {
    let language = page.language.and_then(Language::named);
    let wrap = *page.wrap;
    let body = match *page.status {
        Status::Waiting => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        Status::Failed => {
            status_page(MaterialSymbol::WARNING, tr!("browser.source.unreadable")).any()
        }
        Status::Ready => column_of([
            find_mark(FIND_MARK),
            text_editor(&page.document)
                .read_only(true)
                .wrap(wrap)
                .gutter(true)
                .language(language)
                .search(&page.search)
                .fill()
                .any(),
            find_bar(&page.search)
                .on_action(|page: &mut SourcePage, action: Action| page.search.act(action))
                .anchored_to(FIND_MARK)
                .any(),
        ])
        .fill()
        .any(),
    };
    column! {
        row! {
            text((*page.page).clone()).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            button(text(tr!("browser.source.wrap")))
                .variant(match wrap {
                    true => ButtonVariant::Primary,
                    false => ButtonVariant::Ghost,
                })
                .small()
                .focusless()
                .on_press(|page: &mut SourcePage| page.toggle_wrap());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
        .fill_width();
        body;
    }
    .fill()
    .any()
}
