mod activation;
mod announced;
mod bus;
mod desktop;
mod display_power;
mod environment;
mod gestures;
mod kind;
mod lock;
mod nice;
mod priority;
mod publish;
mod seam;
mod shortcuts;
mod switcher;
mod toplevels;
mod workspaces;

use std::io;
use std::path::PathBuf;

use rikka_compositor_reactor::Reactor;
use rikka_power::LoginSession;
use rikka_seam::Request;
use rikka_session::profile::Profile;
use rikka_session::shortcuts::Shortcut;
use rikka_session::{
    activation as activation_seam, display_power as display_power_seam, gestures as gestures_seam,
    lock as lock_seam, shortcuts as shortcuts_seam, switcher as switcher_seam,
    toplevels as toplevels_seam, workspaces as workspaces_seam,
};

pub(crate) use announced::Announced;
pub(crate) use bus::PrivateBus;
pub(crate) use desktop::{Desktop, FIRST_LOCKER_TIMEOUT, LOCKER_TIMEOUT};
pub(crate) use display_power::Slumber;
pub(crate) use environment::DESKTOP_NAME;
pub(crate) use gestures::Swipes;
pub(crate) use kind::SessionKind;
pub(crate) use lock::Guarded;
pub(crate) use nice::NiceCapability;
pub(crate) use priority::raise_frame_loop;
pub(crate) use seam::SeamHost;
pub(crate) use switcher::{SwitcherRequest, Switches};
pub(crate) use toplevels::ToplevelRequest;

use crate::context::lock_ctx;
use crate::state::Compositor;
use activation::ActivationService;
use desktop::DesktopService;
use display_power::DisplayPowerService;
use gestures::GesturesService;
use lock::LockService;
use shortcuts::ShortcutsService;
use switcher::SwitcherService;
use toplevels::ToplevelsService;
use workspaces::WorkspacesService;

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) seam: SeamHost,
    pub(crate) workspaces: workspaces::Announced,
    pub(crate) toplevels: toplevels::Announced,
    pub(crate) switches: Switches,
    pub(crate) swipes: Option<Swipes>,
    pub(crate) guarded: Guarded,
    pub(crate) slumber: Slumber,
    desktop: Desktop,
    pub(crate) standing: SessionKind,
}

impl Session {
    pub(crate) fn new(
        reactor: &Reactor<Compositor>,
        profile: Profile,
        standing: SessionKind,
        desktop: Desktop,
    ) -> io::Result<Self> {
        let mut seam = SeamHost::new(reactor)?;
        seam.register(desktop::CAPABILITY, DesktopService);
        seam.register(shortcuts_seam::CAPABILITY, ShortcutsService::new());
        let mut workspaces = workspaces::Announced::new(workspaces_seam::CHANGED);
        let mut swipes = None;
        if profile.is_full() {
            if let Some(context) = seam.register(workspaces_seam::CAPABILITY, WorkspacesService) {
                workspaces.attach(context.publisher().clone());
            }
            swipes = seam
                .register(gestures_seam::CAPABILITY, GesturesService)
                .map(|context| Swipes::new(context.publisher().clone()));
        }
        let mut toplevels = toplevels::Announced::new(toplevels_seam::CHANGED);
        if let Some(context) = seam.register(toplevels_seam::CAPABILITY, ToplevelsService) {
            toplevels.attach(context.publisher().clone());
        }
        let mut switches = Switches::new(switcher_seam::CHANGED);
        if let Some(context) = seam.register(switcher_seam::CAPABILITY, SwitcherService) {
            switches.attach(context.publisher().clone());
        }
        seam.register(activation_seam::CAPABILITY, ActivationService);
        let login = standing
            .is_login()
            .then(|| {
                reactor.mailbox(|compositor: &mut Compositor, request| {
                    lock_ctx!(compositor).logind_request(request)
                })
            })
            .transpose()?
            .and_then(|mailbox| {
                LoginSession::watch(move |ask| {
                    let _ = mailbox.send(ask);
                })
                .inspect_err(|error| tracing::warn!("failed to watch logind Lock/Unlock: {error}"))
                .ok()
            });
        let mut guarded = Guarded::new(login);
        if let Some(context) = seam.register(lock_seam::CAPABILITY, LockService) {
            guarded.attach(context.publisher().clone());
        }
        let mut slumber = Slumber::new(display_power_seam::CHANGED);
        if let Some(context) = seam.register(display_power_seam::CAPABILITY, DisplayPowerService) {
            slumber.attach(context.publisher().clone());
        }
        Ok(Self {
            seam,
            workspaces,
            toplevels,
            switches,
            swipes,
            guarded,
            slumber,
            standing,
            desktop,
        })
    }

    pub(crate) fn announce(&mut self, shortcut: Shortcut) {
        self.seam.seam().client().notify(
            Request::new(shortcuts_seam::CAPABILITY, shortcuts_seam::ACTIVATE).encode(&shortcut),
        );
    }
}

pub(crate) fn directory(socket: &str) -> Option<PathBuf> {
    let seam = rikka_seam::net::socket_path_for(socket)?;
    seam.parent().map(std::path::Path::to_path_buf)
}
