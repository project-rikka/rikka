use std::path::{Path, PathBuf};
use std::sync::Arc;

use ade_files::{Change, Created, Finished, Job, Kind, Moved, Root, Update};
use ade_tree::Row;
use rikka_app::prelude::*;

use crate::labels;
use crate::project::act::given;
use crate::project::explorer::clipboard;
use crate::project::explorer::state::{Explorer, Naming};
use crate::project::state::Project;
use crate::project::workspace::WorkspaceParts;

const JOBS: &str = "ade-explorer-jobs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerAction {
    Open,
    NewFile,
    NewFolder,
    Rename,
    Trash,
    Erase,
    Cut,
    Copy,
    Paste,
    Duplicate,
    CopyPath,
    CopyRelativePath,
    Reveal,
    CollapseAll,
}

impl Project {
    pub(in crate::project) fn explore(
        &mut self,
        act: impl FnOnce(&mut Explorer, WorkspaceParts<'_>),
    ) {
        match self.shown_workspace() {
            Some(workspace) => workspace.explore(act),
            None => skip_view(),
        }
    }
}

pub(crate) fn perform(explorer: &mut Explorer, mut parts: WorkspaceParts<'_>, act: ExplorerAction) {
    explorer.dismiss_quietly();
    let chosen = explorer.selection().topmost();
    match act {
        ExplorerAction::Open => {
            let files: Vec<&PathBuf> = chosen
                .iter()
                .filter(|path| !parts.tree.is_folder(path))
                .collect();
            if files.is_empty() {
                match explorer.selection().focus().map(Path::to_path_buf) {
                    Some(folder) => parts.toggle(&folder),
                    None => skip_view(),
                }
                return;
            }
            for path in files {
                parts.viewer.open(path.clone(), true, parts.git);
            }
        }
        ExplorerAction::NewFile | ExplorerAction::NewFolder => {
            let folder = folder_for(explorer, &parts);
            parts.open_up(&folder);
            let kind = match act {
                ExplorerAction::NewFile => Kind::File,
                _ => Kind::Folder,
            };
            explorer.name(Naming::Create {
                folder,
                kind,
                document: TextDocument::new(""),
            });
        }
        ExplorerAction::Rename => {
            let Some(path) = explorer.selection().focus().map(Path::to_path_buf) else {
                skip_view();
                return;
            };
            let name = labels::file_name(&path);
            let document = TextDocument::new(&name);
            let folder = parts.tree.is_folder(&path);
            let stem = name.rfind('.').filter(|&dot| dot > 0 && !folder);
            document.select(0, stem.unwrap_or(name.len()));
            explorer.name(Naming::Rename { path, document });
        }
        ExplorerAction::Trash => trash(explorer, parts.root, &chosen),
        ExplorerAction::Erase => explorer.ask_erasing(chosen),
        ExplorerAction::Cut | ExplorerAction::Copy => {
            clipboard::copy_files(&chosen, act == ExplorerAction::Cut)
        }
        ExplorerAction::Paste => {
            let folder = folder_for(explorer, &parts);
            let root = parts.root.to_owned();
            spawn_blocking(
                clipboard::take_files,
                given(
                    move |project: &mut Project, files: Option<clipboard::ClipboardFiles>| {
                        let (Some(files), Some(workspace)) = (files, project.workspace(&root))
                        else {
                            skip_view();
                            return;
                        };
                        workspace.explore(|explorer, parts| match files.cut {
                            true => move_into(explorer, parts.root, &files.paths, &folder),
                            false => copy_into(explorer, parts.root, &files.paths, &folder),
                        });
                    },
                ),
            );
        }
        ExplorerAction::Duplicate => {
            for path in chosen {
                if let Some(folder) = path.parent() {
                    copy_into(explorer, parts.root, std::slice::from_ref(&path), folder);
                }
            }
        }
        ExplorerAction::CopyPath => {
            rikka_ui::clipboard::set_text(&clipboard::paths_text(&chosen, None))
        }
        ExplorerAction::CopyRelativePath => {
            rikka_ui::clipboard::set_text(&clipboard::paths_text(&chosen, Some(parts.root)));
        }
        ExplorerAction::Reveal => {
            let shown = chosen
                .first()
                .map(|path| match parts.tree.is_folder(path) {
                    true => path.clone(),
                    false => path
                        .parent()
                        .map_or_else(|| path.clone(), Path::to_path_buf),
                })
                .unwrap_or_else(|| parts.root.to_owned());
            if let Err(error) = rikka_open::open(&shown.to_string_lossy()) {
                explorer
                    .report_error(tr!("ade.files.unopened", reason = &error.to_string()).into());
            }
        }
        ExplorerAction::CollapseAll => parts.tree.collapse_all(),
    }
}

pub(crate) fn pressed(
    explorer: &mut Explorer,
    mut parts: WorkspaceParts<'_>,
    row: &Row,
    modifiers: Modifiers,
    presses: u8,
) {
    explorer.dismiss_quietly();
    let path = row.path();
    match (modifiers.control, modifiers.shift) {
        (true, _) => {
            explorer.chosen().toggle(path);
            return;
        }
        (false, true) => {
            explorer.chosen().extend(path, &parts.tree.rows(), false);
            return;
        }
        (false, false) => explorer.chosen().only(path),
    }
    match row.is_folder() {
        true => parts.toggle(path),
        false => parts.viewer.open(path.clone(), presses >= 2, parts.git),
    }
}

pub(crate) fn extend_selection(
    explorer: &mut Explorer,
    parts: WorkspaceParts<'_>,
    path: &Path,
    shift: bool,
) {
    match shift {
        true => explorer.chosen().extend(path, &parts.tree.rows(), false),
        false => explorer.chosen().only(path),
    }
}

pub(crate) fn navigate(explorer: &mut Explorer, mut parts: WorkspaceParts<'_>, into: bool) -> bool {
    let rows = parts.tree.rows();
    let Some(row) = explorer.focused_row(&rows).cloned() else {
        return false;
    };
    match (into, row.is_folder(), row.is_open()) {
        (true, true, false) | (false, true, true) => parts.toggle(row.path()),
        (true, true, true) => {
            let at = rows.iter().position(|listed| listed.path() == row.path());
            let child = at
                .and_then(|at| rows.get(at + 1))
                .filter(|child| child.depth() > row.depth());
            match child {
                Some(child) => explorer.choose(child.path()),
                None => return false,
            }
        }
        (false, _, _) => {
            let parent = row
                .path()
                .parent()
                .filter(|parent| *parent != parts.root)
                .map(Path::to_path_buf);
            match parent {
                Some(parent) => explorer.choose(&parent),
                None => return false,
            }
        }
        (true, false, _) => return false,
    }
    true
}

pub(crate) fn open_context_menu(explorer: &mut Explorer, row: Option<&Row>, at: Point) {
    let pasteable = rikka_ui::clipboard::has_files();
    match row {
        Some(row) if !explorer.selection().contains(row.path()) => {
            explorer.chosen().only(row.path());
        }
        Some(_) => {}
        None => explorer.chosen().clear(),
    }
    explorer.summon(at, row, pasteable);
}

pub(crate) fn submit_name(explorer: &mut Explorer, parts: WorkspaceParts<'_>) {
    if explorer.naming().is_none() || !explorer.commit() {
        skip_view();
        return;
    }
    let Some(naming) = explorer.naming() else {
        return;
    };
    let typed = naming.document().text();
    match naming {
        Naming::Rename { path, .. } => {
            if typed.trim() == labels::file_name(path) || typed.trim().is_empty() {
                explorer.take_naming();
                return;
            }
            let from = path.clone();
            run(
                parts.root,
                move |files| {
                    files.rename(&from, &typed).map(|to| Moved {
                        from: from.clone(),
                        to,
                    })
                },
                |explorer, mut parts, renamed: Moved| {
                    explorer.take_naming();
                    settle_moves(explorer, &mut parts, std::slice::from_ref(&renamed));
                    explorer.choose(&renamed.to);
                },
            );
        }
        Naming::Create { folder, kind, .. } => {
            if typed.trim().is_empty() {
                explorer.take_naming();
                return;
            }
            let (folder, kind) = (folder.clone(), *kind);
            run(
                parts.root,
                move |files| files.create(&folder, &typed, kind),
                |explorer, mut parts, Created { path, kind }: Created| {
                    explorer.take_naming();
                    parts.refresh(path.ancestors().skip(1).map(Path::to_path_buf));
                    parts.reveal(&path);
                    explorer.choose(&path);
                    if kind == Kind::File {
                        parts.viewer.open(path, true, parts.git);
                    }
                },
            );
        }
    }
}

pub(crate) fn name_changed(explorer: &mut Explorer) {
    let Some(naming) = explorer.naming() else {
        skip_view();
        return;
    };
    let typed = naming.document().text();
    let typed = typed.trim();
    let conflict = match naming {
        Naming::Rename { path, .. } => path
            .parent()
            .map(|folder| folder.join(typed))
            .filter(|target| target != path && typed != labels::file_name(path)),
        Naming::Create { folder, .. } => Some(folder.join(typed)),
    }
    .filter(|_| !typed.is_empty())
    .filter(|target| target.symlink_metadata().is_ok());
    match conflict {
        Some(target) => {
            explorer.report_error(tr!("ade.files.taken", name = &labels::file_name(&target)).into())
        }
        None => explorer.forget_trouble(),
    }
}

pub(crate) fn delete_confirmed(explorer: &mut Explorer, parts: WorkspaceParts<'_>) {
    let Some(paths) = explorer.take_erasing() else {
        skip_view();
        return;
    };
    let made = Root::new(parts.root).delete(&paths);
    start(explorer, parts.root, made);
}

pub(crate) fn drop_paths(
    explorer: &mut Explorer,
    parts: WorkspaceParts<'_>,
    sources: Vec<PathBuf>,
    folder: PathBuf,
    moving: bool,
) {
    explorer.aim_at(None);
    let inside = sources.iter().all(|path| path.starts_with(parts.root));
    let sources: Vec<PathBuf> = sources
        .into_iter()
        .filter(|path| path.parent() != Some(folder.as_path()) && *path != folder)
        .collect();
    if sources.is_empty() {
        return;
    }
    match moving && inside {
        true => move_into(explorer, parts.root, &sources, &folder),
        false => copy_into(explorer, parts.root, &sources, &folder),
    }
}

fn trash(explorer: &mut Explorer, root: &Path, paths: &[PathBuf]) {
    if paths.is_empty() {
        skip_view();
        return;
    }
    start(explorer, root, Root::new(root).trash(paths));
}

fn move_into(explorer: &mut Explorer, root: &Path, paths: &[PathBuf], folder: &Path) {
    start(explorer, root, Root::new(root).move_into(paths, folder));
}

fn copy_into(explorer: &mut Explorer, root: &Path, paths: &[PathBuf], folder: &Path) {
    start(explorer, root, Root::new(root).copy_into(paths, folder));
}

fn start(explorer: &mut Explorer, root: &Path, made: ade_files::Result<Job>) {
    let job = match made {
        Ok(job) => job,
        Err(error) => {
            explorer.report_error(labels::files_error(&error));
            return;
        }
    };
    let updates = job.updates();
    let id = explorer.track(job);
    let workspace = root.to_owned();
    stream_keyed(
        (JOBS, workspace.clone(), id),
        updates,
        given(
            move |project, update: Update| match project.workspace(&workspace) {
                Some(workspace) => {
                    workspace.explore(|explorer, parts| advanced(explorer, parts, id, update));
                }
                None => skip_view(),
            },
        ),
    );
}

fn advanced(explorer: &mut Explorer, mut parts: WorkspaceParts<'_>, id: u64, update: Update) {
    let Finished {
        change, failures, ..
    } = match update {
        Update::Working(snapshot) => {
            if !explorer.advance(id, *snapshot) {
                skip_view();
            }
            return;
        }
        Update::Finished(finished) => finished,
    };
    explorer.finish(id);
    match change {
        Change::Moved(moved) => {
            settle_moves(explorer, &mut parts, &moved);
            if let Some(last) = moved.last() {
                parts.reveal(&last.to);
                explorer.choose(&last.to);
            }
        }
        Change::Copied(made) => {
            parts.refresh(
                made.iter()
                    .filter_map(|path| path.parent().map(Path::to_path_buf)),
            );
            if let Some(last) = made.last() {
                parts.reveal(last);
                explorer.choose(last);
            }
        }
        Change::Removed(removed) => settle_removal(explorer, &mut parts, &removed),
    }
    if let Some(message) = labels::files_failures(&failures) {
        explorer.report_error(message);
    }
}

fn settle_moves(explorer: &mut Explorer, parts: &mut WorkspaceParts<'_>, moved: &[Moved]) {
    for Moved { from, to } in moved {
        parts.tree.moved(from, to);
        explorer.moved(from, to);
    }
    parts.viewer.moved(moved, parts.git);
    parts.refresh(moved.iter().flat_map(|Moved { from, to }| {
        [from.parent(), to.parent()]
            .into_iter()
            .flatten()
            .map(Path::to_path_buf)
            .collect::<Vec<_>>()
    }));
}

fn settle_removal(explorer: &mut Explorer, parts: &mut WorkspaceParts<'_>, removed: &[PathBuf]) {
    parts.viewer.removed(removed, parts.git);
    parts.refresh(
        removed
            .iter()
            .filter_map(|path| path.parent().map(Path::to_path_buf)),
    );
    explorer.keep(|path| !removed.iter().any(|removed| path.starts_with(removed)));
}

fn folder_for(explorer: &Explorer, parts: &WorkspaceParts<'_>) -> PathBuf {
    let Some(focus) = explorer.selection().focus() else {
        return parts.root.to_owned();
    };
    match parts.tree.is_folder(focus) {
        true => focus.to_owned(),
        false => focus
            .parent()
            .map_or_else(|| parts.root.to_owned(), Path::to_path_buf),
    }
}

fn run<T: Send + 'static>(
    root: &Path,
    job: impl FnOnce(&Root) -> ade_files::Result<T> + Send + 'static,
    land: impl Fn(&mut Explorer, WorkspaceParts<'_>, T) + 'static,
) {
    let files = Root::new(root);
    let workspace = root.to_owned();
    spawn_blocking(
        move || job(&files).map_err(|error| labels::files_error(&error)),
        given(
            move |project, done: Result<T, Arc<str>>| match project.workspace(&workspace) {
                Some(workspace) => workspace.explore(|explorer, parts| match done {
                    Ok(done) => land(explorer, parts, done),
                    Err(message) => explorer.report_error(message),
                }),
                None => skip_view(),
            },
        ),
    );
}
