use rikka_app::prelude::*;
use rikka_notify::{DEFAULT_ACTION, Icon, Notification};

use crate::alerts::{ActiveNotification, ENTRY, activate};
use crate::shell::Shell;
use browser_model::{DownloadPhase, EndedDownload, download_name};

const SHOW: &str = "show-downloads";

impl Shell {
    pub(crate) fn download_ended(&mut self, ended: &EndedDownload) {
        if self.browser().focused().is_some() {
            return;
        }
        let summary = match ended.phase {
            DownloadPhase::Failed => tr!("browser.downloads.failed"),
            _ => tr!("browser.downloads.finished"),
        };
        let mut desktop = Notification::new(summary.to_string())
            .body(download_name(&ended.item))
            .icon(Icon::named(ENTRY))
            .app_name(tr!("browser.title").to_string())
            .desktop_entry(ENTRY)
            .action(SHOW, tr!("browser.downloads.show").to_string());
        if ended.phase == DownloadPhase::Finished {
            desktop = desktop.default_action(tr!("browser.downloads.open").to_string());
        }
        let shown = ActiveNotification {
            profile: ended.profile,
            download: ended.item.id,
            window: self.id(),
        };
        self.post(desktop, shown);
    }

    pub(crate) fn download_notice_answered(
        &mut self,
        shown: ActiveNotification,
        key: &str,
        token: Option<String>,
    ) {
        match key {
            DEFAULT_ACTION => {
                if let Some(runtime) = self.browser().running_engine(shown.profile) {
                    runtime.downloads().open(shown.download);
                }
            }
            SHOW => {
                let browser = self.browser();
                let shown_in = browser.sender(shown.window).or_else(|| {
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
