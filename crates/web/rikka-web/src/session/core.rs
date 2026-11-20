use rikka_sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use rikka_gecko_protocol::geckoview::session_state::{SessionState, SessionStateUpdate};
use rikka_gecko_protocol::{Target, WindowId};
use rikka_nested::View;

use crate::outbox::Outbox;
use crate::runtime::core::RuntimeCore;
use crate::session::certificates::FailedCertificate;
use crate::session::content::UserContent;
use crate::session::event::{CloseReason, Event};
use crate::session::initial::Initial;
use crate::session::page::PageState;
use crate::session::settings::SessionSettings;
use crate::wire::Link;

#[derive(Debug)]
pub(crate) struct SessionCore {
    pub(crate) window: WindowId,
    pub(crate) link: Arc<Link>,
    pub(crate) runtime: Weak<RuntimeCore>,
    pub(crate) view: View,
    pub(crate) outbox: Outbox<Event>,
    page: Mutex<PageState>,
    settings: Mutex<SessionSettings>,
    content: Mutex<UserContent>,
    saved: Mutex<SessionState>,
    initial: Mutex<Initial>,
    failed_certificate: Mutex<Option<FailedCertificate>>,
    ended: AtomicBool,
}

impl SessionCore {
    pub(crate) fn new(
        window: WindowId,
        link: &Arc<Link>,
        runtime: Weak<RuntimeCore>,
        view: View,
        settings: SessionSettings,
    ) -> SessionCore {
        SessionCore {
            window,
            link: Arc::clone(link),
            runtime,
            view,
            outbox: Outbox::default(),
            page: Mutex::new(PageState::default()),
            settings: Mutex::new(settings),
            content: Mutex::new(UserContent::default()),
            saved: Mutex::new(SessionState::default()),
            initial: Mutex::new(Initial::default()),
            failed_certificate: Mutex::new(None),
            ended: AtomicBool::new(false),
        }
    }

    pub(crate) fn target(&self) -> Target {
        Target::Window(self.window.clone())
    }

    pub(crate) fn page(&self) -> PageState {
        self.page.lock().clone()
    }

    pub(crate) fn change(&self, edit: impl FnOnce(&mut PageState)) {
        let changed = {
            let mut page = self.page.lock();
            let before = page.clone();
            edit(&mut page);
            *page != before
        };
        if changed {
            self.outbox.send(Event::Changed);
        }
    }

    pub(crate) fn settings(&self) -> SessionSettings {
        self.settings.lock().clone()
    }

    pub(crate) fn set_settings(&self, settings: SessionSettings) {
        *self.settings.lock() = settings;
    }

    pub(crate) fn content(&self) -> UserContent {
        self.content.lock().clone()
    }

    pub(crate) fn set_content(&self, content: UserContent) {
        *self.content.lock() = content;
    }

    pub(crate) fn saved(&self) -> SessionState {
        self.saved.lock().clone()
    }

    pub(crate) fn record(&self, update: SessionStateUpdate) {
        let mut saved = self.saved.lock();
        saved.apply(update);
        self.change(|page| page.follow_history(&saved));
    }

    pub(crate) fn ask_load(&self, uri: Option<&str>) {
        self.initial.lock().ask(uri);
    }

    pub(crate) fn allows(&self, uri: &str) -> bool {
        self.initial.lock().allows(uri)
    }

    pub(crate) fn is_loading_initial(&self) -> bool {
        !self.initial.lock().is_past()
    }

    pub(crate) fn certificate_failed(&self, certificate: FailedCertificate) {
        *self.failed_certificate.lock() = Some(certificate);
    }

    pub(crate) fn failed_certificate(&self) -> Option<FailedCertificate> {
        self.failed_certificate.lock().take()
    }

    pub(crate) fn end(&self, reason: CloseReason) {
        if self.ended.swap(true, Ordering::AcqRel) {
            return;
        }
        self.outbox.send(Event::Closed(reason));
        self.outbox.close();
    }

    pub(crate) fn close(&self) {
        if self.ended.swap(true, Ordering::AcqRel) {
            return;
        }
        self.link.close(&self.window);
        self.link.closed(&self.window);
        self.outbox.send(Event::Closed(CloseReason::Requested));
        self.outbox.close();
    }

    pub(crate) fn has_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }
}

impl Drop for SessionCore {
    fn drop(&mut self) {
        if !self.ended.swap(true, Ordering::AcqRel) {
            self.link.close(&self.window);
        }
        self.link.closed(&self.window);
        if let Some(runtime) = self.runtime.upgrade() {
            runtime.remove_window(&self.window);
        }
    }
}
