use std::collections::HashSet;

use browser_pages::Origin;
use rikka_app::prelude::*;
use rikka_notify::{DEFAULT_ACTION, Icon, Invoked, Ledger, Notifications, Picture};
use rikka_web::Notification;

use crate::shell::Shell;
use browser_model::DownloadKey;
use browser_model::{ProfileKey, TabId, WindowId};

pub(crate) const ENTRY: &str = "rikka-browser";

#[derive(Debug, Default)]
pub(crate) struct Alerts {
    notifications: Notifications,
    shown: Ledger<ActiveNotification>,
    sending: HashSet<String>,
    withdrawn: HashSet<String>,
}

#[derive(Debug)]
pub(crate) enum ActiveNotification {
    Page {
        tab: Option<TabId>,
        web: Notification,
    },
    Download {
        item: DownloadKey,
        window: WindowId,
    },
}

impl Alerts {
    pub(crate) fn notifications(&self) -> &Notifications {
        &self.notifications
    }

    fn sending(&mut self, shown: &ActiveNotification) {
        if let ActiveNotification::Page { web, .. } = shown {
            self.sending.insert(web.name.clone());
        }
    }

    fn sent(&mut self, shown: &ActiveNotification) -> bool {
        let ActiveNotification::Page { web, .. } = shown else {
            return true;
        };
        self.sending.remove(&web.name);
        !self.withdrawn.remove(&web.name)
    }

    fn shown(&mut self, id: rikka_notify::Id, shown: ActiveNotification) -> bool {
        if !self.sent(&shown) {
            return false;
        }
        self.shown.record(id, shown);
        true
    }

    fn withdraw(&mut self, name: &str) -> Option<rikka_notify::Id> {
        let notification_id = self.shown.find(
            |shown| matches!(shown, ActiveNotification::Page { web, .. } if web.name == name),
        );
        match notification_id {
            Some(id) => drop(self.shown.remove(id)),
            None if self.sending.contains(name) => {
                self.withdrawn.insert(name.to_owned());
            }
            None => {}
        }
        notification_id
    }
}

impl Shell {
    fn tab_of_origin(&self, key: ProfileKey, origin: Option<&str>) -> Option<TabId> {
        let origin = Origin::parse(origin?)?;
        let model = self.model();
        model.windows.iter().find_map(|layout| {
            layout.tabs().into_iter().find(|tab| {
                model.tab(*tab).is_some_and(|tab_entry| {
                    tab_entry.profile == key
                        && Origin::parse(&tab_entry.meta.url).as_ref() == Some(&origin)
                })
            })
        })
    }

    pub(crate) fn notified(&mut self, key: ProfileKey, web: Notification) {
        let tab = self.tab_of_origin(key, web.origin.as_deref());
        let icon = tab
            .and_then(|tab| (*self.model().tab(tab)?.meta.favicon).clone())
            .and_then(|favicon| Picture::rgba(favicon.width, favicon.height, favicon.rgba).ok());
        let site = web
            .origin
            .as_deref()
            .and_then(Origin::parse)
            .map(|origin| origin.host)
            .unwrap_or_default();
        let body = match (site.is_empty(), web.text.is_empty()) {
            (true, _) => web.text.clone(),
            (false, true) => site,
            (false, false) => format!("{site}\n{}", web.text),
        };
        let mut desktop = rikka_notify::Notification::new(web.title.clone())
            .body(body)
            .app_name(tr!("browser.title").to_string())
            .desktop_entry(ENTRY)
            .default_action(tr!("browser.notification.open").to_string());
        for action in &web.actions {
            desktop = desktop.action(action.action.clone(), action.title.clone());
        }
        desktop = match icon {
            Some(picture) => desktop.icon(picture),
            None => desktop.icon(Icon::named(ENTRY)),
        };
        self.post(desktop, ActiveNotification::Page { tab, web });
    }

    pub(crate) fn post(&mut self, desktop: rikka_notify::Notification, shown: ActiveNotification) {
        let notifications = {
            let mut alerts = self.browser().page_services().alerts_mut();
            alerts.sending(&shown);
            alerts.notifications().clone()
        };
        let sending = notifications.clone();
        spawn(
            async move { (sending.notify(&desktop).await, shown) },
            move |shell: &mut Shell,
                  (sent, shown): (rikka_notify::Result<rikka_notify::Id>, ActiveNotification)| {
                match sent {
                    Ok(id) => {
                        if !shell
                            .browser()
                            .page_services()
                            .alerts_mut()
                            .shown(id, shown)
                        {
                            let closing = notifications.clone();
                            spawn(
                                async move { closing.close(id).await },
                                |_: &mut Shell, _| {},
                            );
                        }
                    }
                    Err(error) => {
                        shell.browser().page_services().alerts_mut().sent(&shown);
                        tracing::warn!(%error, "failed to show notification");
                    }
                }
            },
        );
    }

    pub(crate) fn notification_closed(&mut self, name: &str) {
        let mut alerts = self.browser().page_services().alerts_mut();
        let Some(id) = alerts.withdraw(name) else {
            return;
        };
        let notifications = alerts.notifications().clone();
        drop(alerts);
        spawn(
            async move { notifications.close(id).await },
            |_: &mut Shell, closed: rikka_notify::Result<()>| {
                if let Err(error) = closed {
                    tracing::debug!(%error, "failed to close site notification");
                }
            },
        );
    }

    pub(crate) fn alert_event(&mut self, event: rikka_notify::Event) {
        let browser = std::rc::Rc::clone(self.browser());
        let invoked = browser.page_services().alerts_mut().shown.settle(event);
        let Some(Invoked { shown, key, token }) = invoked else {
            return;
        };
        match shown {
            ActiveNotification::Page { tab, web } if key == DEFAULT_ACTION => {
                web.click(None);
                let owner =
                    tab.and_then(|tab| Some((tab, browser.model().windows.window_of(tab)?)));
                if let Some((tab, sender)) =
                    owner.and_then(|(tab, window)| Some((tab, browser.sender(window)?)))
                {
                    let _ = sender.send(move |shell: &mut Shell| {
                        shell.select(tab);
                        activate(token);
                    });
                }
            }
            ActiveNotification::Page { web, .. } => web.click(Some(&key)),
            ActiveNotification::Download { item, window } => {
                self.download_notice_answered(item, window, &key, token);
            }
        }
    }
}

pub(crate) fn activate(token: Option<String>) {
    if let Some(token) = token {
        request_window(WindowRequest::Activate(token));
    }
}
