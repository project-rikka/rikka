use std::collections::HashMap;

use rikka_notifications::{CloseReason, Notification, Posted};
use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext, Version};
use shell_kit::Host;

use crate::announcement::{Announced, Announcement};
use crate::look::Look;
use crate::shared::Shared;
use crate::stored::StoredNotification;

pub(crate) const CAPABILITY: &str = "rikka.notifications";

pub(crate) const ARRIVED: &str = "arrived";

pub(super) const POST: &str = "post";

pub(crate) const SHOWING: &str = "showing";

pub(crate) const CHANGED: &str = "changed";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Showing {
    pub(crate) shown: usize,
    pub(crate) waiting: usize,
    pub(crate) quiet: bool,
}

#[derive(Debug)]
pub(super) struct NotifyService {
    shared: Shared,
    seam: Option<ServiceContext>,
    tags: HashMap<String, u32>,
}

impl NotifyService {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> NotifyService {
        NotifyService {
            shared,
            seam: None,
            tags: HashMap::new(),
        }
    }

    fn arrived(&mut self) {
        for posted in self.shared.action_router.drain() {
            match posted {
                Posted::Shown(notification) => self.show(*notification),
                Posted::Withdrawn(id) => self.withdraw(id),
            }
        }
        self.settle();
    }

    fn show(&mut self, notification: Notification) {
        if notification.progress.is_some() && self.shared.history.contains(notification.id) {
            let look = self.look(&notification);
            return self
                .shared
                .history
                .refresh(StoredNotification::new(notification, look));
        }
        self.shared.history.release(notification.id);
        if self.shared.preferences.keeps_out(notification.urgency) {
            tracing::debug!("do not disturb: queued `{}`", notification.summary);
            self.shared.stack.dismiss(notification.id);
            let look = self.look(&notification);
            return self.miss(StoredNotification::new(notification, look));
        }
        let linger = self.shared.preferences.linger(&notification);
        let look = self.look(&notification);
        if let Some(dropped) = self.shared.stack.post(notification, linger, look) {
            tracing::debug!("notification backlog full: queued {}", dropped.id());
            self.miss(dropped.stored_notification());
        }
    }

    fn miss(&mut self, missed: StoredNotification) {
        if missed.notification.transient {
            self.shared
                .action_router
                .close(missed.id(), CloseReason::Expired);
            return;
        }
        self.shared.history.keep(missed);
    }

    #[must_use]
    fn look(&self, notification: &Notification) -> Look {
        Look::for_notification(notification, &self.shared.apps)
    }

    fn post(&mut self, call: Call) {
        let announcement = match call.decode::<Announcement>() {
            Ok(announcement) => announcement,
            Err(error) => return call.fail(error),
        };
        let Announced {
            tag,
            mut notification,
            action_requests,
        } = announcement.into_announced();
        let router = &self.shared.action_router;
        match tag.as_ref().and_then(|tag| self.tags.get(tag)) {
            Some(&id) if router.is_open(id) => notification.id = id,
            Some(_) if notification.progress.is_some() => return call.done(),
            _ => {}
        }
        let id = router.post(notification, action_requests);
        if let Some(tag) = tag {
            self.tags.retain(|_, tagged| router.is_open(*tagged));
            self.tags.insert(tag, id);
        }
        self.arrived();
        call.done();
    }

    fn withdraw(&mut self, id: u32) {
        self.shared.action_router.close(id, CloseReason::Withdrawn);
        self.shared.stack.dismiss(id);
        self.shared.history.release(id);
    }

    fn settle(&mut self) {
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(CHANGED, &())
        {
            tracing::warn!("failed to emit screen state change: {error}");
        }
    }

    #[must_use]
    fn showing(&self) -> Showing {
        let (shown, waiting) = self
            .shared
            .stack
            .read(|shown, waiting| (shown.len(), waiting));
        Showing {
            shown,
            waiting,
            quiet: self
                .shared
                .preferences
                .keeps_out(rikka_notifications::Urgency::Normal),
        }
    }
}

impl Service<Host> for NotifyService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, Version::new(1, 0))
            .summary("Desktop notifications")
            .method(MethodInfo::new(ARRIVED).summary("Notification received"))
            .method(MethodInfo::new(POST).summary("Posts a desktop notification"))
            .method(MethodInfo::new(SHOWING).summary("Returns shown and queued notifications"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("Shown notifications changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
        self.arrived();
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            ARRIVED => {
                self.arrived();
                call.done();
            }
            POST => self.post(call),
            SHOWING => call.encode(&self.showing()),
            _ => call.unknown_method(),
        }
    }
}
