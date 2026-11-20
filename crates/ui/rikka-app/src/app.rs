use rikka_appearance::Appearance;
use rikka_i18n::I18nBuilder;
use rikka_instance::{LaunchRequest, Startup};
use rikka_theme::Theme;
use rikka_ui_host::VideoPlayback;

use crate::backend;
use crate::device::Device;
use crate::error::Result;
use crate::instance::{Inbox, Instance};
use crate::launch::Launch;
use crate::tasks;
use crate::window::{IntoWindows, Opening};

#[must_use]
pub struct App {
    name: String,
    device: Device,
    videos: Option<Box<dyn VideoPlayback>>,
    theme: Option<Theme>,
    i18n: Option<I18nBuilder>,
    instance: Option<Instance>,
    #[cfg(unix)]
    arriving: Option<String>,
    windows: Vec<Opening>,
}

impl App {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: crate::instance::identity(name.into()),
            device: Device::default(),
            videos: None,
            theme: None,
            i18n: None,
            instance: None,
            #[cfg(unix)]
            arriving: backend::token::arriving_token(),
            windows: Vec::new(),
        }
    }

    pub fn draws_3d(mut self) -> Self {
        self.device.three = true;
        self
    }

    pub fn decodes_video(mut self) -> Self {
        self.device.video = true;
        self
    }

    pub fn plays_video(mut self, videos: impl VideoPlayback + 'static) -> Self {
        self.device.video = true;
        self.videos = Some(Box::new(videos));
        self
    }

    pub fn computes(mut self) -> Self {
        self.device.compute = true;
        self
    }

    pub fn computes_in_background(mut self) -> Self {
        self.device.compute = true;
        self.device.background = true;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn i18n(mut self, i18n: I18nBuilder) -> Self {
        self.i18n = Some(i18n);
        self
    }

    pub fn instance(mut self, instance: Instance) -> Self {
        self.instance = Some(instance);
        self
    }

    pub fn windows<W: IntoWindows>(self, make: impl Fn(LaunchRequest) -> W + 'static) -> Self {
        let first = make(LaunchRequest::from_arguments(std::env::args_os()));
        self.instance(Instance::windows(make)).window(first)
    }

    pub fn window(mut self, window: impl IntoWindows) -> Self {
        self.windows.extend(window.into_windows());
        self
    }

    pub fn run(mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        rikka_heap::return_large_glibc_blocks();
        rikka_log::install("warn");
        let name = self.name;
        let inbox = Inbox::default();
        let policy = self.instance.take().map(|instance| instance.policy);
        #[cfg(unix)]
        let arriving = self.arriving.filter(|_| policy.is_some());
        let mut primary_service = None;
        if policy.is_some() {
            match rikka_instance::claim(&name, inbox.sink()) {
                Startup::Primary(service) => primary_service = Some(service),
                Startup::Running(peer) => {
                    #[cfg(unix)]
                    let request = forwarded_request(&name, arriving.clone());
                    #[cfg(windows)]
                    let request = LaunchRequest::from_arguments(std::env::args_os());
                    if forward_to_running_instance(&name, &peer, &request) {
                        return Ok(());
                    }
                }
                Startup::Standalone => {}
            }
        }

        #[cfg(unix)]
        let language = self.i18n.is_some();
        if let Some(i18n) = self.i18n.take() {
            i18n.embedded(rikka_widgets::CATALOGS).build()?.install();
            #[cfg(unix)]
            backend::settings::apply_session_locale();
        }
        let (notifier, wake) = rikka_worker::event_fd()?;
        let (proxy, mailbox) = tasks::mailbox(notifier);
        let (theme, appearance) = match self.theme.take() {
            Some(theme) => (theme, None),
            None => {
                let worn = Appearance::current();
                (worn.theme(), Some(worn))
            }
        };
        crate::preload::fonts(&theme);
        inbox.install(proxy.clone());
        let outcome = backend::run(Launch {
            app_name: name,
            device: self.device,
            videos: self.videos,
            theme,
            appearance,
            #[cfg(unix)]
            language,
            windows: self.windows,
            policy,
            #[cfg(unix)]
            arriving,
            proxy,
            mailbox,
            wake,
        });
        drop(primary_service);
        outcome
    }
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("name", &self.name)
            .field("instance", &self.instance)
            .field("windows", &self.windows)
            .finish_non_exhaustive()
    }
}

#[cfg(unix)]
fn forwarded_request(name: &str, arriving: Option<String>) -> LaunchRequest {
    let request = LaunchRequest::from_arguments(std::env::args_os());
    match backend::token::token_for(name, arriving) {
        Some(token) => request.with_token(token),
        None => request,
    }
}

fn forward_to_running_instance(
    name: &str,
    peer: &rikka_instance::Peer,
    request: &LaunchRequest,
) -> bool {
    match peer.forward(request) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!("failed to forward arguments to running `{name}`: {error}");
            false
        }
    }
}
