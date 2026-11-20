use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ade_files::Moved;
use ade_git::State;
use ade_tree::rebased;
use rikka_app::prelude::*;
use rikka_file_edit::{Mark, Marks, TextFile};
use rikka_ui::search::{Action, Search};

use crate::labels;
use crate::project::act::given;
use crate::project::git::Git;

const DWELL: Duration = Duration::from_millis(150);
const REMEMBERED: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Basis {
    head: Option<String>,
    fresh: bool,
}

impl Basis {
    fn for_path(git: &Git, path: &Path) -> Basis {
        Basis {
            head: git.status().head().map(str::to_owned),
            fresh: !git.status().is_born()
                || git
                    .changed()
                    .file(path)
                    .is_some_and(|state| matches!(state, State::Untracked | State::Added)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closing {
    Others,
    Right,
    Saved,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    Save,
    Discard,
    Keep,
}

#[derive(Debug)]
pub(crate) struct Shown {
    path: PathBuf,
    document: TextDocument,
    sheet: Option<TextFile>,
    base: Option<Arc<str>>,
    basis: Option<Basis>,
    edits: Edits,
    place: Place,
    search: Search,
    error_message: Option<Arc<str>>,
    stale: bool,
    saving: Option<u64>,
    pinned: bool,
    reading: u64,
}

impl Shown {
    fn new(path: PathBuf, pinned: bool) -> Shown {
        Shown {
            path,
            document: TextDocument::new(""),
            sheet: None,
            base: None,
            basis: None,
            edits: Edits::default(),
            place: Place::new(),
            search: Search::new(),
            error_message: None,
            stale: false,
            saving: None,
            pinned,
            reading: 0,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn document(&self) -> &TextDocument {
        &self.document
    }

    pub(crate) fn edits(&self) -> Edits {
        self.edits.clone()
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn search(&self) -> &Search {
        &self.search
    }

    pub(crate) fn is_finding(&self) -> bool {
        self.search.is_open()
    }

    pub(crate) fn is_pinned(&self) -> bool {
        self.pinned
    }

    fn open_find(&self, replacing: bool) {
        let selection = self.document.selected_text();
        let seed = Some(selection.as_str()).filter(|selection| !selection.is_empty());
        match replacing {
            true => self.search.open_replacing(seed),
            false => self.search.open(seed),
        }
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn is_readable(&self) -> bool {
        self.sheet.is_some()
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.saving.is_some()
    }

    pub(crate) fn is_stale(&self) -> bool {
        self.stale
    }

    pub(crate) fn is_unsaved(&self) -> bool {
        self.document.is_modified()
    }

    fn took(&mut self, sheet: TextFile) -> bool {
        let unchanged = self.error_message.is_none()
            && !self.stale
            && !self.document.is_modified()
            && self
                .sheet
                .as_ref()
                .is_some_and(|open| open.content() == sheet.content());
        if !unchanged {
            self.document.reset(sheet.content());
        }
        self.sheet = Some(sheet);
        self.error_message = None;
        self.stale = false;
        !unchanged
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TabMenu {
    pub(crate) tab: usize,
    pub(crate) at: Point,
    pub(crate) key: u64,
}

#[derive(Debug)]
pub(crate) struct Viewer {
    root: PathBuf,
    tabs: Vec<Shown>,
    active: Option<usize>,
    menu: Option<TabMenu>,
    menus: u64,
    recent: Vec<PathBuf>,
    asking: Option<PathBuf>,
    closing_once_saved: Option<PathBuf>,
    reading: u64,
    basing: u64,
    marking: u64,
    typed: u64,
}

impl Viewer {
    pub(crate) fn new(root: PathBuf) -> Viewer {
        Viewer {
            root,
            tabs: Vec::new(),
            active: None,
            menu: None,
            menus: 0,
            recent: Vec::new(),
            asking: None,
            closing_once_saved: None,
            reading: 0,
            basing: 0,
            marking: 0,
            typed: 0,
        }
    }

    pub(crate) fn tabs(&self) -> &[Shown] {
        &self.tabs
    }

    pub(crate) fn active(&self) -> Option<usize> {
        self.active
    }

    pub(crate) fn shown(&self) -> Option<&Shown> {
        self.tabs.get(self.active?)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn menu(&self) -> Option<TabMenu> {
        self.menu
    }

    pub(crate) fn summon(&mut self, tab: usize, at: Point) {
        self.menus += 1;
        self.menu = Some(TabMenu {
            tab,
            at,
            key: self.menus,
        });
    }

    pub(crate) fn dismiss(&mut self) {
        if self.menu.take().is_none() {
            skip_view();
        }
    }

    pub(crate) fn put_menu_away(&mut self) {
        self.menu = None;
    }

    pub(crate) fn recent(&self) -> &[PathBuf] {
        &self.recent
    }

    pub(crate) fn asking(&self) -> Option<&Shown> {
        let asking = self.asking.as_deref()?;
        self.tabs.iter().find(|shown| shown.path == asking)
    }

    pub(crate) fn is_unsaved(&self, path: &Path) -> bool {
        self.tab(path).is_some_and(Shown::is_unsaved)
    }

    fn at(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|shown| shown.path == path)
    }

    fn tab(&self, path: &Path) -> Option<&Shown> {
        self.tabs.iter().find(|shown| shown.path == path)
    }

    fn tab_mut(&mut self, path: &Path) -> Option<&mut Shown> {
        self.tabs.iter_mut().find(|shown| shown.path == path)
    }

    fn shown_mut(&mut self) -> Option<&mut Shown> {
        self.tabs.get_mut(self.active?)
    }

    pub(crate) fn open_find(&mut self, replacing: bool) {
        if let Some(shown) = self.shown() {
            shown.open_find(replacing);
        }
    }

    pub(crate) fn find(&mut self, action: Action) {
        if let Some(shown) = self.shown() {
            shown.search.act(action);
        }
    }

    pub(crate) fn reload(&mut self, git: &Git) {
        if let Some(path) = self.shown().map(|shown| shown.path.clone()) {
            self.reread(&path, true);
            self.rebase(git);
        }
    }

    pub(crate) fn stirred(&mut self, folders: &BTreeSet<PathBuf>, git: &Git) {
        let stirred: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter(|shown| {
                shown
                    .path
                    .parent()
                    .is_some_and(|folder| folders.contains(folder))
            })
            .map(|shown| shown.path.clone())
            .collect();
        if stirred.is_empty() {
            return;
        }
        for path in &stirred {
            self.reread(path, false);
        }
        self.rebase(git);
    }

    pub(crate) fn rebase(&mut self, git: &Git) {
        let Some(shown) = self.shown_mut() else {
            return;
        };
        let basis = Basis::for_path(git, &shown.path);
        if shown.basis.as_ref() == Some(&basis) {
            return;
        }
        let path = shown.path.clone();
        let fresh = basis.fresh;
        shown.basis = Some(basis);

        self.basing += 1;
        let request = self.basing;
        let Some(repo) = git.repo().cloned() else {
            self.based(&path, request, None);
            return;
        };
        if fresh {
            self.based(&path, request, Some(Arc::from("")));
            return;
        }
        let at = path.clone();
        spawn_blocking(
            move || match repo.at_head(&at) {
                Ok(base) => Some(Arc::from(base)),
                Err(error) => {
                    tracing::debug!(%error, path = %at.display(), "failed to read file at HEAD");
                    None
                }
            },
            landing(&self.root, move |viewer, _, base: Option<Arc<str>>| {
                viewer.based(&path, request, base);
            }),
        );
    }

    pub(crate) fn open(&mut self, path: PathBuf, pin: bool, git: &Git) {
        if !path.starts_with(&self.root) {
            skip_view();
            return;
        }
        if let Some(at) = self.at(&path) {
            let shown = &mut self.tabs[at];
            shown.pinned |= pin;
            self.show(at, git);
            return;
        }

        let shown = Shown::new(path.clone(), pin);
        let preview = self
            .tabs
            .iter()
            .position(|shown| !shown.pinned && !shown.is_unsaved());
        let at = match preview {
            Some(at) => {
                self.tabs[at] = shown;
                at
            }
            None => {
                let at = self.active.map_or(self.tabs.len(), |active| active + 1);
                self.tabs.insert(at, shown);
                at
            }
        };
        self.reread(&path, false);
        self.show(at, git);
    }

    fn show(&mut self, at: usize, git: &Git) {
        let Some(path) = self.tabs.get(at).map(|shown| shown.path.clone()) else {
            self.active = None;
            return;
        };
        self.active = Some(at);
        self.recent.retain(|known| *known != path);
        self.recent.insert(0, path);
        self.recent.truncate(REMEMBERED);
        self.rebase(git);
        self.remark();
    }

    pub(crate) fn activate(&mut self, at: usize, git: &Git) {
        if self.active == Some(at) {
            skip_view();
            return;
        }
        self.show(at, git);
    }

    pub(crate) fn cycle(&mut self, by: isize, git: &Git) {
        let count = self.tabs.len() as isize;
        let Some(active) = self.active.filter(|_| count > 1) else {
            skip_view();
            return;
        };
        let next = (active as isize + by).rem_euclid(count) as usize;
        self.show(next, git);
    }

    pub(crate) fn pin(&mut self, at: usize) {
        match self.tabs.get_mut(at) {
            Some(shown) if !shown.pinned => shown.pinned = true,
            _ => skip_view(),
        }
    }

    pub(crate) fn close(&mut self, at: usize, git: &Git) {
        let Some(shown) = self.tabs.get(at) else {
            skip_view();
            return;
        };
        if shown.is_unsaved() {
            self.asking = Some(shown.path.clone());
            self.show(at, git);
            return;
        }
        self.drop_tab(at, git);
    }

    pub(crate) fn close_active(&mut self, git: &Git) {
        match self.active {
            Some(at) => self.close(at, git),
            None => skip_view(),
        }
    }

    pub(crate) fn close_many(&mut self, which: Closing, at: usize, git: &Git) {
        let kept_path = self.tabs.get(at).map(|shown| shown.path.clone());
        let doomed: Vec<PathBuf> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(index, shown)| {
                !shown.is_unsaved()
                    && match which {
                        Closing::Others => *index != at,
                        Closing::Right => *index > at,
                        Closing::Saved | Closing::All => true,
                    }
            })
            .map(|(_, shown)| shown.path.clone())
            .collect();
        let active = self.shown().map(|shown| shown.path.clone());
        self.tabs.retain(|shown| !doomed.contains(&shown.path));
        let after = match which {
            Closing::Others | Closing::Right => kept_path,
            Closing::Saved | Closing::All => active,
        };
        let next = after
            .and_then(|path| self.at(&path))
            .or_else(|| (!self.tabs.is_empty()).then_some(0));
        match next {
            Some(next) => self.show(next, git),
            None => self.active = None,
        }
        if which == Closing::All
            && let Some(unsaved) = self.tabs.iter().position(Shown::is_unsaved)
        {
            self.close(unsaved, git);
        }
    }

    pub(crate) fn answer(&mut self, answer: Answer, git: &Git) {
        let Some(path) = self.asking.take() else {
            skip_view();
            return;
        };
        let Some(at) = self.at(&path) else {
            return;
        };
        match answer {
            Answer::Keep => {}
            Answer::Discard => self.drop_tab(at, git),
            Answer::Save => {
                self.closing_once_saved = Some(path.clone());
                self.save_at(at);
            }
        }
    }

    fn drop_tab(&mut self, at: usize, git: &Git) {
        let closed = self.tabs.remove(at);
        if self.asking.as_deref() == Some(closed.path.as_path()) {
            self.asking = None;
        }
        if self.tabs.is_empty() {
            self.active = None;
            return;
        }
        let was_active = self.active == Some(at);
        if !was_active {
            if let Some(active) = self.active.filter(|active| *active > at) {
                self.active = Some(active - 1);
            }
            return;
        }
        let next = self
            .recent
            .iter()
            .find_map(|path| self.at(path))
            .unwrap_or(at.min(self.tabs.len() - 1));
        self.show(next, git);
    }

    pub(crate) fn moved(&mut self, moved: &[Moved], git: &Git) {
        for Moved { from, to } in moved {
            for shown in &mut self.tabs {
                if let Some(files) = rebased(&shown.path, from, to) {
                    shown.path = files;
                    shown.basis = None;
                }
            }
            for path in &mut self.recent {
                if let Some(files) = rebased(path, from, to) {
                    *path = files;
                }
            }
        }
        self.rebase(git);
    }

    pub(crate) fn removed(&mut self, paths: &[PathBuf], git: &Git) {
        let under = |path: &Path| paths.iter().any(|paths| path.starts_with(paths));
        self.recent.retain(|path| !under(path));
        let active = self.shown().map(|shown| shown.path.clone());
        let lost: Arc<str> = tr!("ade.file.gone").into();
        for shown in &mut self.tabs {
            if under(&shown.path) && shown.is_unsaved() {
                shown.stale = true;
                shown.error_message = Some(Arc::clone(&lost));
            }
        }
        self.tabs
            .retain(|shown| !under(&shown.path) || shown.is_unsaved());
        let next = active
            .and_then(|path| self.at(&path))
            .or_else(|| self.recent.iter().find_map(|path| self.at(path)))
            .or_else(|| (!self.tabs.is_empty()).then_some(0));
        match next {
            Some(next) => self.show(next, git),
            None => self.active = None,
        }
    }

    fn reread(&mut self, path: &Path, over: bool) {
        self.reading += 1;
        let request = self.reading;
        let Some(shown) = self.tab_mut(path) else {
            return;
        };
        shown.reading = request;
        let path = path.to_owned();
        let at = path.clone();
        spawn_blocking(
            move || TextFile::read(&at).map_err(|error| labels::file_error(&error)),
            landing(
                &self.root,
                move |viewer, _, read: Result<TextFile, Arc<str>>| {
                    viewer.sheet_read(&path, request, over, read);
                },
            ),
        );
    }

    fn sheet_read(
        &mut self,
        path: &Path,
        request: u64,
        over: bool,
        read: Result<TextFile, Arc<str>>,
    ) {
        let active = self.shown().is_some_and(|shown| shown.path == path);
        let Some(shown) = self.tab_mut(path).filter(|shown| shown.reading == request) else {
            skip_view();
            return;
        };

        if !over && shown.is_unsaved() {
            let stale = match &read {
                Ok(fresh) => shown
                    .sheet
                    .as_ref()
                    .is_none_or(|previous| previous.content() != fresh.content()),
                Err(_) => true,
            };
            if stale == shown.stale {
                skip_view();
                return;
            }
            shown.stale = stale;
            return;
        }

        match read {
            Ok(sheet) => {
                if !shown.took(sheet) {
                    skip_view();
                    return;
                }
            }
            Err(message) => {
                shown.sheet = None;
                shown.error_message = Some(message);
            }
        }
        if active {
            self.remark();
        }
    }

    fn based(&mut self, path: &Path, request: u64, base: Option<Arc<str>>) {
        if request != self.basing {
            return;
        }
        let Some(shown) = self.shown_mut().filter(|shown| shown.path == path) else {
            return;
        };
        if base.is_none() {
            shown.basis = None;
        }
        shown.base = base;
        self.remark();
    }

    pub(crate) fn typed(&mut self) {
        if let Some(shown) = self.shown_mut() {
            shown.pinned = true;
        }
        self.typed += 1;
        let request = self.typed;
        after(
            DWELL,
            landing(&self.root, move |viewer, _, (): ()| {
                viewer.save_settled(request)
            })
            .map(|()| ((),)),
        );
    }

    fn save_settled(&mut self, request: u64) {
        match request == self.typed {
            true => self.remark(),
            false => skip_view(),
        }
    }

    fn remark(&mut self) {
        let Some(shown) = self.shown() else {
            return;
        };
        let Some(base) = shown.base.clone().filter(|_| shown.is_readable()) else {
            return;
        };
        let path = shown.path.clone();
        let now = shown.document.text();

        self.marking += 1;
        let request = self.marking;
        spawn_blocking(
            move || Marks::diff(&base, &now),
            landing(&self.root, move |viewer, _, marks: Marks| {
                viewer.marked(&path, request, marks);
            }),
        );
    }

    fn marked(&mut self, path: &Path, request: u64, marks: Marks) {
        if request != self.marking {
            skip_view();
            return;
        }
        let Some(shown) = self.tab_mut(path) else {
            skip_view();
            return;
        };
        let edits = edits(&marks);
        if shown.edits == edits {
            skip_view();
            return;
        }
        shown.edits = edits;
    }

    pub(crate) fn save(&mut self) {
        match self.active {
            Some(at) => self.save_at(at),
            None => skip_view(),
        }
    }

    fn save_at(&mut self, at: usize) {
        let Some(shown) = self.tabs.get_mut(at) else {
            skip_view();
            return;
        };
        let Some(sheet) = shown.sheet.clone() else {
            skip_view();
            return;
        };
        if shown.saving.is_some() || !shown.is_unsaved() {
            skip_view();
            return;
        }

        shown.saving = Some(shown.document.revision());
        shown.error_message = None;
        let content = shown.document.text();
        let path = shown.path.clone();
        spawn_blocking(
            move || {
                let mut sheet = sheet;
                sheet
                    .write(&content)
                    .map(|()| sheet)
                    .map_err(|error| labels::file_error(&error))
            },
            landing(
                &self.root,
                move |viewer, git, wrote: Result<TextFile, Arc<str>>| {
                    viewer.saved(&path, wrote, git);
                },
            ),
        );
    }

    fn saved(&mut self, path: &Path, wrote: Result<TextFile, Arc<str>>, git: &Git) {
        let Some(shown) = self.tab_mut(path) else {
            skip_view();
            return;
        };
        let saved_revision = shown.saving.take();

        let closing = match wrote {
            Ok(sheet) => {
                shown.sheet = Some(sheet);
                shown.stale = false;
                if saved_revision == Some(shown.document.revision()) {
                    shown.document.mark_saved();
                }
                !shown.is_unsaved()
            }
            Err(message) => {
                shown.stale = true;
                shown.error_message = Some(message);
                false
            }
        };
        if self.closing_once_saved.as_deref() != Some(path) {
            return;
        }
        self.closing_once_saved = None;
        if closing && let Some(at) = self.at(path) {
            self.drop_tab(at, git);
        }
    }
}

fn landing<T: 'static>(
    root: &Path,
    land: impl Fn(&mut Viewer, &Git, T) + 'static,
) -> Handler<(T,)> {
    let root = root.to_owned();
    Handler::new(given(move |project, outcome: T| {
        match project.workspace(&root) {
            Some(workspace) => workspace.file(|viewer, git| land(viewer, git, outcome)),
            None => skip_view(),
        }
    }))
}

fn edits(marks: &Marks) -> Edits {
    Edits::new(marks.lines().iter().map(|(line, mark)| {
        let edit = match mark {
            Mark::Added => Edit::Added,
            Mark::Changed => Edit::Changed,
            Mark::Removed => Edit::Gone,
        };
        (line.saturating_sub(1), edit)
    }))
}
