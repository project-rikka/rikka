use std::io;
use std::path::{Path, PathBuf};

use rikka_color_formats::{Document, Grouping, Rgb, export};
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_dirs::{UserDir, UserDirs};
use rikka_seam::{Request, SeamClient};
use rikka_settings::session::color_picker::Format;
use shell_kit::{OPEN, detached};
use shell_notifications::Announcement;

use crate::CAPABILITY;

const THREAD: &str = "rikka-color-export";

#[derive(Debug, thiserror::Error)]
enum ExportError {
    #[error("file chooser failed: {0}")]
    Chooser(#[from] rikka_dialog::Error),
    #[error("failed to write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

#[derive(Clone, Debug)]
pub(crate) struct Strings {
    pub(crate) app_name: String,
    pub(crate) title: String,
    pub(crate) stem: String,
    pub(crate) text: String,
    pub(crate) json: String,
    pub(crate) failed: String,
}

#[derive(Debug)]
pub(super) struct Exporting {
    pub(crate) colors: Vec<Rgb>,
    pub(crate) formats: Vec<Format>,
    pub(super) grouping: Grouping,
    pub(crate) words: Strings,
    pub(crate) seam: SeamClient,
}

pub(crate) fn start(exporting: Exporting) {
    detached(THREAD, move || {
        if let Err(error) = save(&exporting) {
            tracing::warn!(%error, "failed to export colours");
            Announcement::new(&exporting.words.app_name, &exporting.words.failed)
                .body(error.to_string())
                .urgent()
                .send(&exporting.seam);
        }
        exporting.seam.notify(Request::new(CAPABILITY, OPEN));
    });
}

fn save(exporting: &Exporting) -> Result<(), ExportError> {
    let words = &exporting.words;
    let mut chooser = Chooser::save(&words.title)
        .current_name(format!("{}.{}", words.stem, Document::Text.extension()))
        .filter(
            Filter::named(&words.text)
                .mime_type("text/plain")
                .pattern("*.txt"),
        )
        .filter(
            Filter::named(&words.json)
                .mime_type("application/json")
                .pattern("*.json"),
        );
    if let Some(documents) = UserDirs::load().path(UserDir::Documents) {
        chooser = chooser.current_folder(documents);
    }
    let path = match chooser.run()? {
        ChooserResponse::Files(files) => match files.into_iter().next() {
            Some(path) => path,
            None => return Ok(()),
        },
        ChooserResponse::Cancelled => return Ok(()),
    };
    let (path, document) = typed(path);
    let exported = export(
        &exporting.colors,
        &exporting.formats,
        exporting.grouping,
        document,
    );
    rikka_fs::replace(&path, exported).map_err(|source| ExportError::Write { path, source })
}

fn typed(path: PathBuf) -> (PathBuf, Document) {
    let document = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(Document::from_extension);
    match document {
        Some(document) => (path, document),
        None => (with_text_extension(&path), Document::Text),
    }
}

fn with_text_extension(path: &Path) -> PathBuf {
    let mut named = path.as_os_str().to_owned();
    named.push(".");
    named.push(Document::Text.extension());
    PathBuf::from(named)
}
