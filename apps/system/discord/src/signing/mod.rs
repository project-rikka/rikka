mod portal;
mod view;

use discord_client::{Method, QrStep, SignInStep};
use discord_core::Picture;
use rikka_app::prelude::*;
use rikka_barcode::QrCode;
use rikka_ui::ImageSource;
use rikka_web::{CloseReason, Event, Session};

use portal::Portal;
pub(crate) use view::view;

const PORTAL: &str = "discord-sign-in-portal";
const CODE_MODULE: u32 = 6;

#[derive(Clone, Debug)]
pub(crate) enum Step {
    Starting,
    Code(ImageSource),
    Scanned {
        name: String,
        avatar: Option<Picture>,
    },
    Page,
    Stopped,
}

#[derive(State)]
pub(crate) struct Signing {
    open: Tracked<bool>,
    method: Tracked<Option<Method>>,
    step: Tracked<Step>,
    error_message: Tracked<Option<String>>,
    #[untracked]
    attempt: u64,
    #[untracked]
    running: bool,
    #[untracked]
    portal: Option<Portal>,
}

impl Signing {
    pub(crate) fn new() -> Signing {
        Signing {
            open: Tracked::new(false),
            method: Tracked::new(None),
            step: Tracked::new(Step::Starting),
            error_message: Tracked::new(None),
            attempt: 0,
            running: false,
            portal: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        *self.open
    }

    pub(crate) fn method(&self) -> Option<Method> {
        *self.method
    }

    pub(crate) fn step(&self) -> &Step {
        &self.step
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn portal_session(&self) -> Option<&Session> {
        self.portal.as_ref().map(Portal::session)
    }

    pub(crate) fn is_attempt(&self, attempt: u64) -> bool {
        self.running && self.attempt == attempt
    }

    pub(crate) fn open(&mut self) {
        self.drop_attempt();
        self.open.set(true);
        self.error_message.set(None);
    }

    pub(crate) fn close(&mut self) {
        self.drop_attempt();
        self.open.set(false);
        self.error_message.set(None);
    }

    pub(crate) fn begin(&mut self, method: Method) -> u64 {
        self.stop();
        self.attempt += 1;
        self.running = true;
        self.error_message.set(None);
        self.method.set(Some(method));
        *self.step = Step::Starting;
        self.attempt
    }

    pub(crate) fn fail(&mut self, error_message: String) {
        self.stop();
        *self.step = Step::Stopped;
        self.error_message.set(Some(error_message));
    }

    pub(crate) fn prompted(&mut self, attempt: u64, step: SignInStep) {
        if !self.is_attempt(attempt) {
            return;
        }
        match step {
            SignInStep::Qr(QrStep::Code(code)) => match QrCode::encode(&code) {
                Ok(code) => {
                    let image = code.rasterize(CODE_MODULE);
                    *self.step = Step::Code(ImageSource::Rgba {
                        width: image.side(),
                        height: image.side(),
                        pixels: image.into_rgba().into(),
                    });
                }
                Err(error) => {
                    tracing::warn!("failed to draw sign-in code: {error}");
                    self.error_message
                        .set(Some(tr!("discord.signing.qr-failed").to_string()));
                }
            },
            SignInStep::Qr(QrStep::Scanned { name, avatar }) => {
                *self.step = Step::Scanned { name, avatar };
            }
            SignInStep::Web(request) => match Portal::open(request) {
                Ok(portal) => {
                    if let Some(events) = portal.session().events() {
                        stream_keyed(PORTAL, events, |signing: &mut Signing, event: Event| {
                            signing.handle_portal_event(event);
                        });
                    }
                    self.portal = Some(portal);
                    *self.step = Step::Page;
                }
                Err(reason) => self.error_message.set(Some(reason)),
            },
        }
    }

    fn handle_portal_event(&mut self, event: Event) {
        skip_view();
        let Some(portal) = &self.portal else {
            return;
        };
        match event {
            Event::NewSession(opening) => portal.follow(opening),
            Event::ScriptMessage(message) => {
                let Some(posted) = portal.posted(&message) else {
                    return;
                };
                if let Some(portal) = self.portal.take() {
                    cancel(PORTAL);
                    portal.finish(posted);
                    *self.step = Step::Starting;
                }
            }
            Event::Closed(CloseReason::EngineExited) => {
                self.fail(tr!("discord.signing.page-closed").to_string());
            }
            _ => {}
        }
    }

    fn close_portal(&mut self) {
        cancel(PORTAL);
        self.portal = None;
    }

    fn stop(&mut self) {
        self.close_portal();
        self.running = false;
    }

    fn drop_attempt(&mut self) {
        self.stop();
        self.method.set(None);
        *self.step = Step::Starting;
    }
}

impl std::fmt::Debug for Signing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Signing")
            .field("open", &*self.open)
            .field("method", &*self.method)
            .finish_non_exhaustive()
    }
}
