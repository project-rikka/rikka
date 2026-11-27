use browser_translators::probe_script;
use rikka_app::prelude::*;
use rikka_web::{Account, NewSessionRequest, Opening, RuntimeEvent};

use crate::chrome::Phase;
use crate::shell::events::adopt;
use crate::shell::{AdoptedSession, Shell, WindowContent, window};
use browser_model::{ProfileKey, WindowId};

const RUNTIME_FEED: &str = "rikka-browser-runtime-events";
const ALERT_FEED: &str = "rikka-browser-notification-events";
const ACCOUNT: &str = "rikka-browser-account";

impl Shell {
    pub(crate) fn listen(&mut self) {
        let alive = self.browser().windows();
        let window = self.id();
        let page_services = self.browser().page_services();
        for (key, events) in page_services.subscribe_runtimes(window, &alive) {
            stream_keyed(
                (RUNTIME_FEED, key),
                events,
                move |shell: &mut Shell, event: RuntimeEvent| shell.runtime_event(key, event),
            );
        }
        if let Some(events) = page_services.subscribe_alerts(window, &alive) {
            stream_keyed(ALERT_FEED, events, |shell: &mut Shell, event| {
                shell.alert_event(event)
            });
        }
    }

    fn runtime_event(&mut self, key: ProfileKey, event: RuntimeEvent) {
        let chrome = &self.browser().services().chrome;
        match event {
            RuntimeEvent::Ready { version } => {
                tracing::info!(%version, "Chrome is ready");
                self.engine_ready(key);
            }
            RuntimeEvent::Failed(failure) => {
                tracing::error!(?failure, "Chrome cannot be driven");
                chrome.set_phase(key, Phase::Failed(failure));
            }
            RuntimeEvent::Exited(exit) => {
                tracing::warn!(?exit, "Chrome exited");
                chrome.set_phase(key, Phase::Exited(exit));
                self.browser().page_services().forget(key);
            }
            RuntimeEvent::NewSession(request) => self.engine_opens(key, request),
            RuntimeEvent::ForeignWindow(foreign) => open_window(crate::devtools::window(foreign)),
            RuntimeEvent::Downloads(items) => self.downloads_changed(key, items),
            RuntimeEvent::Visited(_) | RuntimeEvent::VisitsRemoved { .. } => {
                self.browser().history_changed();
            }
            RuntimeEvent::BookmarksChanged => {
                if let ProfileKey::Persistent(profile) = key {
                    self.sync_bookmarks(profile);
                }
            }
            RuntimeEvent::ExtensionsChanged => self.list_extensions_of(key),
            RuntimeEvent::AccountChanged(account) => chrome.set_account(key, account),
            _ => {}
        }
    }

    fn engine_ready(&mut self, key: ProfileKey) {
        self.browser()
            .services()
            .chrome
            .set_phase(key, Phase::Ready);
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        runtime.register(&probe_script());
        runtime.downloads().hide_chrome_bubble();
        spawn_keyed(
            (ACCOUNT, key),
            runtime.account(),
            move |shell: &mut Shell, account: rikka_web::Result<Option<Account>>| match account {
                Ok(account) => shell.browser().services().chrome.set_account(key, account),
                Err(error) => tracing::debug!(%error, "failed to read the Google account"),
            },
        );
        self.list_extensions_of(key);
        if let ProfileKey::Persistent(profile) = key {
            self.sync_bookmarks(profile);
        }
    }

    fn engine_opens(&mut self, key: ProfileKey, request: NewSessionRequest) {
        let address = request.url.clone();
        let opening = request.opening;
        let Some(live) = adopt(request) else {
            return;
        };
        match self.window_for(key) {
            Some(target) if target == self.id() => self.adopt_opened(live, address, opening),
            Some(target) => self.raise(target, move |shell: &mut Shell| {
                shell.adopt_opened(live, address, opening);
            }),
            None => {
                let content = match opening {
                    Opening::Popup { width, height } => WindowContent::Little(
                        Some(key),
                        crate::little::Little::Popup(
                            AdoptedSession { live, address },
                            crate::little::PopupSize { width, height },
                        ),
                    ),
                    _ => WindowContent::Adopted(AdoptedSession { live, address }),
                };
                open_window(window(self.browser(), key == ProfileKey::Private, content));
            }
        }
    }

    fn window_for(&self, key: ProfileKey) -> Option<WindowId> {
        let browser = self.browser();
        let model = browser.model();
        let serves = |window: WindowId| {
            model
                .window(window)
                .is_some_and(|layout| !layout.little && model.profile_here(window) == key)
                && browser.sender(window).is_some()
        };
        browser
            .focused()
            .filter(|window| serves(*window))
            .or_else(|| model.windows.main().filter(|window| serves(*window)))
            .or_else(|| {
                model
                    .windows
                    .iter()
                    .map(|layout| layout.id)
                    .find(|window| serves(*window))
            })
    }
}
