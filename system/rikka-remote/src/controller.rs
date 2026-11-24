use std::sync::mpsc::Sender;

use rikka_seam::{Call, Publisher, SeamClient};
use rikka_session::remote::{CHANGED, RemoteStatus};
use rikka_settings::seam::{Announcement, Settings};

use crate::options::Options;
use crate::server::Server;
use crate::status::{StatusChange, StatusSink};

pub(crate) enum Message {
    Call(Box<Call>),
    Settings(Announcement),
    Language(Announcement),
    Status(StatusChange),
    SessionEnded,
}

pub(crate) struct Controller {
    settings: Settings,
    options: Options,
    compositor: SeamClient,
    runtime: tokio::runtime::Handle,
    messages: Sender<Message>,
    server: Option<Server>,
    status: RemoteStatus,
    publisher: Option<Publisher>,
}

impl Controller {
    pub(crate) fn new(
        settings: Settings,
        compositor: SeamClient,
        runtime: tokio::runtime::Handle,
        messages: Sender<Message>,
    ) -> Controller {
        let options = Options::read(&settings);
        let mut controller = Controller {
            settings,
            options: options.clone(),
            compositor,
            runtime,
            messages,
            server: None,
            status: RemoteStatus::default(),
            publisher: None,
        };
        controller.start(&options);
        controller
    }

    pub(crate) fn publish_with(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
        self.publish();
    }

    pub(crate) fn status(&self) -> &RemoteStatus {
        &self.status
    }

    pub(crate) fn settings_changed(&mut self, announcement: Announcement) {
        match announcement {
            Announcement::Changed(changed) => {
                if !self.settings.apply_change(&changed) {
                    return;
                }
            }
            Announcement::Lagged => {
                if let Err(error) = self.settings.refresh() {
                    tracing::warn!("cannot reload settings: {error}");
                }
            }
        }
        let next = Options::read(&self.settings);
        if next == self.options {
            return;
        }
        let restart =
            next.enabled != self.options.enabled || next.listening != self.options.listening;
        if restart {
            self.stop();
            self.start(&next);
        } else if let Some(server) = &self.server {
            server.set_media(next.media.clone());
        }
        self.options = next;
    }

    pub(crate) fn status_changed(&mut self, change: StatusChange) {
        match change {
            StatusChange::Viewer(viewer) => self.status.viewer = viewer,
            StatusChange::Problem(problem) => self.status.problem = problem,
        }
        self.publish();
    }

    pub(crate) fn disconnect(&self, call: Call) {
        if let Some(server) = &self.server {
            server.disconnect();
        }
        call.encode(&());
    }

    pub(crate) fn stop(&mut self) {
        self.server = None;
        self.status = RemoteStatus::default();
        self.publish();
    }

    fn start(&mut self, options: &Options) {
        if !options.enabled {
            return;
        }
        let status = StatusSink::new(self.messages.clone());
        match Server::start(options, &self.runtime, status, self.compositor.clone()) {
            Ok(server) => {
                self.status.serving = Some(server.serving());
                self.status.problem = None;
                self.server = Some(server);
            }
            Err(error) => {
                tracing::error!("cannot start the remote desktop: {error}");
                self.status.problem = Some(error.to_string());
            }
        }
        self.publish();
    }

    fn publish(&self) {
        let Some(publisher) = &self.publisher else {
            return;
        };
        if let Err(error) = publisher.emit(CHANGED, &self.status) {
            tracing::debug!("cannot publish status: {error}");
        }
    }
}
