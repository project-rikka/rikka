use files_core::Location;
use files_vfs::Task as Job;
use rikka_clipboard::Offer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopiedFiles {
    pub(crate) items: Vec<Location>,
    pub(crate) cut: bool,
}

pub(crate) fn publish(items: &[Location], cut: bool) {
    if items.is_empty() {
        return;
    }
    if let Err(error) = rikka_ui::clipboard::publish(offer(items, cut)) {
        tracing::warn!(%error, "failed to copy files to clipboard");
    }
}

pub(crate) fn holds_files() -> bool {
    rikka_ui::clipboard::has_files()
}

pub(crate) fn paste(into: Location) -> Option<Job> {
    let clipboard_files = clipboard_files()?;
    if clipboard_files.cut {
        rikka_ui::clipboard::publish(Offer::new()).ok();
    }
    let sources = clipboard_files.items;
    Some(match clipboard_files.cut {
        true => Job::Move { sources, into },
        false => Job::Copy { sources, into },
    })
}

fn clipboard_files() -> Option<CopiedFiles> {
    let files = rikka_ui::clipboard::files()?;
    let items: Vec<Location> = files
        .uris
        .iter()
        .filter_map(|uri| Location::parse(uri))
        .collect();
    (!items.is_empty()).then_some(CopiedFiles {
        items,
        cut: files.cut,
    })
}

fn offer(items: &[Location], cut: bool) -> Offer {
    let uris: Vec<&str> = items.iter().map(Location::as_str).collect();
    let written = items
        .iter()
        .map(|at| {
            at.to_path()
                .map_or_else(|| at.as_str().to_owned(), |path| path.display().to_string())
        })
        .collect::<Vec<_>>()
        .join("\n");
    Offer::files(&uris, cut).with_text(written)
}
