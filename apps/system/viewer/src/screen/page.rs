use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_markdown::{markdown, markdown_text};
use rikka_syntax::Language;
use rikka_ui::search::{Action, Hits, Search, Tally};
use rikka_widgets::{column, find_bar, find_mark, segmented, text_editor};
use viewer_text::{TextFile, WriteReport};

use crate::bar;
use crate::screen::{Screen, status};
use crate::viewer::Viewer;

const PREVIEW: usize = 0;
const SOURCE: usize = 1;

const FIND: &str = "viewer-find";

#[derive(Clone, Copy, Debug)]
struct Meta {
    encoding: &'static str,
    bom: bool,
    truncated: bool,
}

#[derive(Debug)]
pub(crate) struct Page {
    path: PathBuf,
    document: TextDocument,
    read: Option<Meta>,
    error: Option<Label>,
    markdown: bool,
    preview: bool,
    saving: bool,
    saved: Option<io::Result<WriteReport>>,
    source: RefCell<Option<(u64, Arc<str>)>>,
    plain: RefCell<Option<(u64, Arc<str>)>>,
    search: Search,
    hits: Hits,
}

impl Page {
    pub(crate) fn open(path: &Path, markdown: bool) -> Self {
        Self {
            path: path.to_path_buf(),
            document: TextDocument::default(),
            read: None,
            error: None,
            markdown,
            preview: markdown,
            saving: false,
            saved: None,
            source: RefCell::new(None),
            plain: RefCell::new(None),
            search: Search::new(),
            hits: Hits::default(),
        }
    }

    pub(crate) fn start(&self) {
        let path = self.path.clone();
        spawn_blocking(
            move || TextFile::read(&path),
            |screen: &mut Screen, read| {
                if let Some(page) = screen.page() {
                    page.read(read);
                }
            },
        );
    }

    fn read(&mut self, read: io::Result<TextFile>) {
        match read {
            Ok(file) => {
                self.document.reset(&file.text);
                self.read = Some(Meta {
                    encoding: file.encoding,
                    bom: file.bom,
                    truncated: file.truncated,
                });
            }
            Err(error) => self.error = Some(status::unreadable(&error)),
        }
        self.refresh_find();
    }

    pub(crate) fn save(&mut self) {
        let Some(read) = self.read.filter(|_| self.savable()) else {
            return;
        };
        self.saving = true;
        let path = self.path.clone();
        let text = self
            .document
            .line_ending()
            .join(&self.document.text())
            .into_owned();
        spawn_blocking(
            move || TextFile::write(&path, &text, read.encoding, read.bom),
            |screen: &mut Screen, report| {
                if let Some(page) = screen.page() {
                    page.saved(report);
                }
            },
        );
    }

    fn savable(&self) -> bool {
        self.read.is_some_and(|read| !read.truncated) && !self.saving && self.document.is_modified()
    }

    fn saved(&mut self, report: io::Result<WriteReport>) {
        self.saving = false;
        if report.is_ok() {
            self.document.mark_saved();
        }
        self.saved = Some(report);
    }

    pub(crate) fn unsaved(&self) -> bool {
        self.document.is_modified()
    }

    fn show(&mut self, view: usize) {
        self.preview = view == PREVIEW;
        self.refresh_find();
    }

    pub(crate) fn open_find(&mut self, replacing: bool) {
        let selection = self.document.selected_text();
        let seed = Some(selection.as_str()).filter(|selection| !selection.is_empty());
        match replacing && !self.preview {
            true => self.search.open_replacing(seed),
            false => self.search.open(seed),
        }
        self.refresh_find();
    }

    pub(crate) fn find(&mut self, action: Action) {
        self.search.act(action);
        self.refresh_find();
    }

    pub(crate) fn is_finding(&self) -> bool {
        self.search.is_open()
    }

    fn refresh_find(&mut self) {
        if !self.preview {
            return;
        }
        let query = self.search.query();
        if !self.search.is_open() || !query.is_readable() {
            self.hits.forget();
            self.search.report(Tally::default());
            return;
        }
        let plain = self.plain();
        self.hits.refresh(&query, &plain);
        self.hits.follow(self.search.take_step());
        self.search.report(self.hits.tally());
    }

    pub(crate) fn stage(&self) -> Element {
        if let Some(error) = &self.error {
            return status_page(MaterialSymbol::ERROR, error.clone())
                .grow(1.0)
                .build();
        }
        if self.read.is_none() {
            return container(spinner()).center().grow(1.0).build();
        }
        column! {
            find_mark(FIND);
            page_stack(self.at(), self.body()).grow(1.0).fill_width();
            find_bar(&self.search)
                .anchored_to(FIND)
                .replaceable(!self.preview)
                .on_action(|page: &mut Page, action| page.find(action));
        }
        .grow(1.0)
        .fill_width()
        .build()
    }

    fn at(&self) -> usize {
        match self.preview {
            true => PREVIEW,
            false => SOURCE,
        }
    }

    fn body(&self) -> Element {
        if self.preview {
            let source = self.source();
            let base = self.path.parent().unwrap_or(Path::new("."));
            return scrollable(
                container(
                    markdown(source)
                        .on_link(|viewer: &mut Viewer, target: String| viewer.follow(&target))
                        .found(self.search.found().current(self.hits.current()))
                        .base(base)
                        .fill_width(),
                )
                .padding(Inset::xy(16.0, 20.0)),
            )
            .grow(1.0)
            .build();
        }
        text_editor(&self.document)
            .on_change(|| {})
            .read_only(self.read.is_some_and(|read| read.truncated))
            .wrap(self.markdown)
            .language(Language::from_path(&self.path))
            .search(&self.search)
            .autofocus(true)
            .grow(1.0)
            .build()
    }

    fn plain(&self) -> Arc<str> {
        let revision = self.document.revision();
        let mut cache = self.plain.borrow_mut();
        if let Some((was, plain)) = cache.as_ref()
            && *was == revision
        {
            return Arc::clone(plain);
        }
        let plain = markdown_text(&self.source());
        *cache = Some((revision, Arc::clone(&plain)));
        plain
    }

    fn source(&self) -> Arc<str> {
        let revision = self.document.revision();
        let mut cache = self.source.borrow_mut();
        if let Some((was, source)) = cache.as_ref()
            && *was == revision
        {
            return Arc::clone(source);
        }
        let source: Arc<str> = Arc::from(self.document.text());
        *cache = Some((revision, Arc::clone(&source)));
        source
    }

    pub(crate) fn detail(&self) -> Option<Element> {
        let read = self.read.as_ref()?;
        let palette = palette();
        Some(
            row! {
                bar::detail(read.encoding, palette.text_muted);
                bar::detail(
                    tr!("viewer.text.lines", count = self.document.line_count()),
                    palette.text_muted,
                );
                if !self.preview {
                    let (line, column) = self.document.caret_line_column();
                    bar::detail(
                        tr!("viewer.text.caret", line = line + 1, column = column + 1),
                        palette.text_muted,
                    );
                }
                if read.truncated {
                    bar::detail(tr!("viewer.text.truncated"), palette.warning);
                }
                self.state();
                bar::icon_button(
                    MaterialSymbol::SAVE,
                    |page: &mut Page| page.save(),
                    self.savable(),
                );
                if self.markdown {
                    segmented(self.at())
                        .segments([tr!("viewer.text.preview"), tr!("viewer.text.source")])
                        .control_size(ControlSize::Small)
                        .on_select(|page: &mut Page, view| page.show(view));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build(),
        )
    }

    fn state(&self) -> Option<impl View> {
        let palette = palette();
        if self.saving {
            return Some(bar::detail(tr!("viewer.text.saving"), palette.text_muted));
        }
        if self.document.is_modified() {
            return Some(bar::detail(tr!("viewer.text.modified"), palette.warning));
        }
        match self.saved.as_ref()? {
            Err(error) => Some(bar::detail(
                tr!("viewer.text.unsaved", reason = error.to_string().as_str()),
                palette.danger,
            )),
            Ok(report) if report.lossy => Some(bar::detail(
                tr!("viewer.text.lossy", encoding = report.encoding),
                palette.warning,
            )),
            Ok(report) if report.encoding != self.read.as_ref()?.encoding => Some(bar::detail(
                tr!("viewer.text.converted", encoding = report.encoding),
                palette.warning,
            )),
            Ok(_) => None,
        }
    }
}
