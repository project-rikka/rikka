mod button;
mod notice;
mod panel;

pub(crate) use button::{BUTTON, button};
pub(crate) use panel::{newest_first, panel, row as item_row};

use rikka_web::{DownloadId, DownloadItem, Downloads};

use crate::shell::Shell;
use browser_model::{DownloadPhase, ProfileKey};

impl Shell {
    pub(crate) fn downloads_changed(&mut self, key: ProfileKey, items: Vec<DownloadItem>) {
        let (started, ended) = {
            let page_services = self.browser().page_services();
            let baseline = page_services.first_snapshot(key);
            let mut downloads = page_services.downloads_mut();
            let known: Vec<DownloadId> = downloads.items(key).iter().map(|item| item.id).collect();
            let started = !baseline
                && items
                    .iter()
                    .any(|item| !known.contains(&item.id) && !DownloadPhase::of(item).ended());
            (started, downloads.update(key, items))
        };
        for ended in ended {
            self.download_ended(&ended);
        }
        if started {
            self.reveal_downloads(key);
        }
    }

    fn reveal_downloads(&mut self, key: ProfileKey) {
        let browser = std::rc::Rc::clone(self.browser());
        let window = browser
            .focused()
            .filter(|window| browser.model().profile_here(*window) == key)
            .unwrap_or(self.id());
        if window == self.id() {
            self.show_downloads();
        } else if let Some(sender) = browser.sender(window) {
            let _ = sender.send(|shell: &mut Shell| shell.show_downloads());
        }
    }

    fn with_downloads(&self, act: impl FnOnce(Downloads)) {
        let key = self.model().profile_here(self.id());
        match self.browser().running_engine(key) {
            Some(runtime) => act(runtime.downloads()),
            None => tracing::debug!("downloads need a running Chrome"),
        }
    }

    pub(crate) fn open_download(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.open(id));
    }

    pub(crate) fn show_download_folder(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.show(id));
    }

    pub(crate) fn cancel_download(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.cancel(id));
    }

    pub(crate) fn pause_download(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.pause(id));
    }

    pub(crate) fn resume_download(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.resume(id));
    }

    pub(crate) fn delete_download_file(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.delete_file(id));
    }

    pub(crate) fn forget_download(&mut self, id: DownloadId) {
        self.with_downloads(|downloads| downloads.erase(id));
    }

    pub(crate) fn clear_downloads(&mut self) {
        let key = self.model().profile_here(self.id());
        let ended: Vec<DownloadId> = self
            .browser()
            .page_services()
            .downloads()
            .items(key)
            .iter()
            .filter(|item| DownloadPhase::of(item).ended())
            .map(|item| item.id)
            .collect();
        self.with_downloads(|downloads| {
            for id in ended {
                downloads.erase(id);
            }
        });
    }
}
