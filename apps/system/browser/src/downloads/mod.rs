mod button;
mod notice;
mod panel;

pub(crate) use button::{BUTTON, button};
pub(crate) use panel::{panel, row as item_row};

use browser_model::{DownloadItem, DownloadKey};
use std::path::{Path, PathBuf};

use browser_store::{Download, NewDownload};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_format::{safe_file_name, unique_name};
use rikka_web::{DownloadId, DownloadRequest};

use crate::shell::Shell;
use browser_model::{ProfileKey, TabId};
use browser_model::{ProgressUpdate, StartedDownload, StoredDownload};

const KEPT: usize = 200;
const UNNAMED: &str = "download";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DownloadAction {
    Cancel,
    Pause,
    Resume,
    Remove,
}

impl Shell {
    pub(crate) fn download_asked(&mut self, tab: TabId, request: DownloadRequest) {
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            request.cancel();
            return;
        };
        let name = safe_file_name(&request.filename).unwrap_or_else(|| UNNAMED.to_owned());
        let total = request.length.filter(|bytes| *bytes > 0);
        let mime = request.mime_type.clone().unwrap_or_default();
        let stored = match key {
            ProfileKey::Persistent(profile) => Some(self.browser().store().record_download(
                profile,
                NewDownload {
                    url: request.uri.clone(),
                    path: PathBuf::new(),
                    mime: mime.clone(),
                    total,
                },
            )),
            ProfileKey::Private => None,
        };
        let item = self
            .browser()
            .page_services()
            .downloads_mut()
            .begin(StartedDownload {
                profile: key,
                url: request.uri.clone(),
                name: name.clone(),
                mime,
                total,
                stored,
            });
        let preferences = self.browser().preferences();
        let folder = preferences.download_folder();
        let asks = preferences.download_asks();
        self.reveal_download(tab);
        if !asks {
            let path = {
                let page_services = self.browser().page_services();
                let downloads = page_services.downloads();
                folder.join(unique_name(&name, |candidate| {
                    let path = folder.join(candidate);
                    downloads.claims(&path) || path.exists()
                }))
            };
            self.save_download(item, request, path);
            return;
        }
        let chooser = Chooser::save(tr!("browser.downloads.save-title").to_string())
            .parent(window_identifier())
            .current_name(name)
            .current_folder(folder);
        spawn_blocking(
            move || (request, chooser.run()),
            move |shell: &mut Shell,
                  (request, chosen): (
                DownloadRequest,
                rikka_dialog::Result<ChooserResponse>,
            )| {
                match chosen {
                    Ok(ChooserResponse::Files(files)) if !files.is_empty() => {
                        let path = files.into_iter().next().unwrap_or_default();
                        shell.save_download(item, request, path);
                    }
                    chosen => {
                        if let Err(error) = chosen {
                            tracing::warn!(%error, "failed to choose download location");
                        }
                        request.cancel();
                        shell.forget_download(item);
                    }
                }
            },
        );
    }

    fn save_download(&mut self, item: DownloadKey, request: DownloadRequest, path: PathBuf) {
        if let Err(error) = request.save_to(&path) {
            tracing::warn!(%error, "failed to record download");
            self.forget_download(item);
            return;
        }
        let stored = self
            .browser()
            .page_services()
            .downloads_mut()
            .chosen(item, &path);
        self.store_download(stored);
    }

    pub(crate) fn download_changed(&mut self, download: rikka_web::Download) {
        let update_progress = self
            .browser()
            .page_services()
            .downloads_mut()
            .update_progress(&download);
        let Some(ProgressUpdate {
            item,
            stored,
            finished,
        }) = update_progress
        else {
            return;
        };
        self.store_download(stored);
        if finished {
            self.download_finished(item);
        }
    }

    pub(crate) fn download_removed(&mut self, id: &DownloadId) {
        self.browser().page_services().downloads_mut().unlink(id);
    }

    fn store_download(&mut self, stored: Option<StoredDownload>) {
        let Some(StoredDownload { id, progress, path }) = stored else {
            return;
        };
        let store = self.browser().store();
        if let Some(path) = path {
            store.set_download_path(id, path);
        }
        store.update_download(id, progress);
    }

    fn reveal_download(&mut self, tab: TabId) {
        let window = self.model().windows.window_of(tab).unwrap_or(self.id());
        if window == self.id() {
            self.show_downloads();
        } else if let Some(sender) = self.browser().sender(window) {
            let _ = sender.send(|shell: &mut Shell| shell.show_downloads());
        }
    }

    pub(crate) fn load_downloads(&mut self) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        if !self
            .browser()
            .page_services()
            .downloads_mut()
            .needs_loading(profile)
        {
            return;
        }
        let store = self.browser().store();
        spawn_blocking(
            move || {
                store.flush()?;
                store.downloads(profile, KEPT)
            },
            |shell: &mut Shell, stored: browser_store::Result<Vec<Download>>| match stored {
                Ok(stored) => shell
                    .browser()
                    .page_services()
                    .downloads_mut()
                    .merge(stored),
                Err(error) => tracing::warn!(%error, "failed to read download history"),
            },
        );
    }

    fn download_item(&self, item: DownloadKey) -> Option<DownloadItem> {
        self.browser()
            .page_services()
            .downloads()
            .item(item)
            .cloned()
    }

    pub(crate) fn open_download(&mut self, item: DownloadKey) {
        if let Some(path) = self.download_item(item).and_then(|item| item.path) {
            open_path(&path);
        }
    }

    pub(crate) fn show_download_folder(&mut self, item: DownloadKey) {
        let folder = self
            .download_item(item)
            .and_then(|item| Some(item.path?.parent()?.to_owned()));
        if let Some(folder) = folder {
            open_path(&folder);
        }
    }

    fn apply_action(&mut self, item: &DownloadItem, action: DownloadAction) {
        let Some(web) = item.web.clone() else {
            return;
        };
        let Some(runtime) = self.browser().running_engine(item.profile) else {
            return;
        };
        spawn(
            async move {
                let downloads = runtime.downloads();
                match action {
                    DownloadAction::Cancel => downloads.cancel(&web).await,
                    DownloadAction::Pause => downloads.pause(&web).await,
                    DownloadAction::Resume => downloads.resume(&web).await,
                    DownloadAction::Remove => downloads.remove(&web).await,
                }
            },
            move |_: &mut Shell, applied: rikka_web::Result<()>| {
                if let Err(error) = applied {
                    tracing::warn!(%error, ?action, "download action failed");
                }
            },
        );
    }

    pub(crate) fn cancel_download(&mut self, item: DownloadKey) {
        if let Some(download) = self.download_item(item) {
            self.apply_action(&download, DownloadAction::Cancel);
        }
    }

    pub(crate) fn pause_download(&mut self, item: DownloadKey) {
        if let Some(download) = self.download_item(item) {
            self.apply_action(&download, DownloadAction::Pause);
        }
    }

    pub(crate) fn resume_download(&mut self, item: DownloadKey) {
        if let Some(download) = self.download_item(item) {
            self.apply_action(&download, DownloadAction::Resume);
        }
    }

    pub(crate) fn forget_download(&mut self, item: DownloadKey) {
        let removed = self.browser().page_services().downloads_mut().remove(item);
        let Some(removed) = removed else {
            return;
        };
        self.apply_action(&removed, DownloadAction::Remove);
        if let DownloadKey::StoredDownload(id) = removed.key {
            self.browser().store().remove_download(id);
        }
    }

    pub(crate) fn clear_downloads(&mut self) {
        let key = self.model().profile_here(self.id());
        let ended = self
            .browser()
            .page_services()
            .downloads_mut()
            .clear_ended(key);
        for item in &ended {
            self.apply_action(item, DownloadAction::Remove);
        }
        if let ProfileKey::Persistent(profile) = key {
            self.browser().store().clear_finished_downloads(profile);
        }
    }
}

fn open_path(path: &Path) {
    if let Err(error) = rikka_open::open(&path.to_string_lossy()) {
        tracing::warn!(%error, path = %path.display(), "failed to open download");
    }
}
