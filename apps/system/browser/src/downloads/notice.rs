use rikka_app::prelude::*;
use rikka_notify::{DEFAULT_ACTION, Icon, Notification};

use crate::alerts::{ActiveNotification, ENTRY, activate};
use crate::shell::Shell;
use browser_model::DownloadKey;
use browser_model::WindowId;

const SHOW: &str = "show-downloads";

impl Shell {
    pub(crate) fn download_finished(&mut self, item: DownloadKey) {
        if self.browser().focused().is_some() {
            return;
        }
        let Some(name) = self
            .browser()
            .page_services()
            .downloads()
            .item(item)
            .map(|download| download.name.clone())
        else {
            return;
        };
        let desktop = Notification::new(tr!("browser.downloads.finished").to_string())
            .body(name)
            .icon(Icon::named(ENTRY))
            .app_name(tr!("browser.title").to_string())
            .desktop_entry(ENTRY)
            .default_action(tr!("browser.downloads.open").to_string())
            .action(SHOW, tr!("browser.downloads.show").to_string());
        let window = self.id();
        self.post(desktop, ActiveNotification::Download { item, window });
    }

    pub(crate) fn download_notice_answered(
        &mut self,
        item: DownloadKey,
        window: WindowId,
        key: &str,
        token: Option<String>,
    ) {
        match key {
            DEFAULT_ACTION => self.open_download(item),
            SHOW => {
                let browser = self.browser();
                let shown_in = browser.sender(window).or_else(|| {
                    browser
                        .model()
                        .windows
                        .main()
                        .and_then(|main| browser.sender(main))
                });
                if let Some(sender) = shown_in {
                    let _ = sender.send(move |shell: &mut Shell| {
                        shell.show_downloads();
                        activate(token);
                    });
                }
            }
            _ => {}
        }
    }
}
