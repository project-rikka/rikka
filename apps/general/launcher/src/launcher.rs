use std::sync::Arc;

use launcher_auth::{Accounts, Authenticator};
use launcher_instance::Instance;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::accounts::{self, SignIn};
use crate::discover::{self, Discover, ProjectPage, WorldPick};
use crate::instance::{self, InstancePage};
use crate::library::{self, Creating};
use crate::logs::Logs;
use crate::preferences::Preferences;
use crate::runs::Runs;
use crate::services::{Services, USER_AGENT};
use crate::settings::{self, SettingsPage};
use crate::sidebar;
use crate::skins::{self, Skins};

pub(crate) const STAGE: &str = "launcher-stage";
const DIALOG: f32 = 440.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Page {
    Library,
    Instance,
    Discover,
    Project,
    Skins,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) message: String,
    pub(crate) failed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Confirm {
    DeleteInstance(String),
    DeleteWorld {
        instance: String,
        folder: String,
        name: String,
    },
    RemoveContent {
        instance: String,
        key: String,
        title: String,
    },
    RemoveServer {
        instance: String,
        address: String,
        name: String,
    },
    DeleteScreenshot {
        path: std::path::PathBuf,
    },
    RemoveAccount {
        id: String,
        name: String,
    },
}

#[derive(State)]
pub(crate) struct Launcher {
    #[untracked]
    pub(crate) sender: Sender<Launcher>,
    pub(crate) services: Option<Arc<Services>>,
    pub(crate) fatal: Option<String>,
    pub(crate) page: Page,
    pub(crate) previous: Page,
    pub(crate) preferences: Preferences,
    pub(crate) instances: Vec<Instance>,
    pub(crate) selected: Option<String>,
    pub(crate) creating: Option<Creating>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) notice: Option<Notice>,
    pub(crate) authenticator: Option<Arc<Authenticator>>,
    pub(crate) accounts: Accounts,
    pub(crate) signing: Option<SignIn>,
    pub(crate) account_menu: bool,
    pub(crate) runs: Runs,
    #[state]
    pub(crate) logs: Logs,
    pub(crate) instance: InstancePage,
    pub(crate) discover: Discover,
    pub(crate) project: Option<ProjectPage>,
    pub(crate) world_pick: Option<WorldPick>,
    pub(crate) skins: Skins,
    pub(crate) settings: SettingsPage,
}

impl std::fmt::Debug for Launcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Launcher")
            .field("page", &self.page)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl Launcher {
    pub(crate) fn new(sender: Sender<Launcher>) -> Launcher {
        let (services, fatal) = match Services::open() {
            Ok(services) => (Some(Arc::new(services)), None),
            Err(error) => (None, Some(error)),
        };
        let mut notice = None;
        let mut report = |result: Result<(), String>| {
            if let Err(message) = result {
                notice = Some(Notice {
                    message,
                    failed: true,
                });
            }
        };
        let preferences = match &services {
            Some(services) => match Preferences::load(&services.preferences_file()) {
                Ok(preferences) => preferences,
                Err(error) => {
                    report(Err(error));
                    Preferences::default()
                }
            },
            None => Preferences::default(),
        };
        let accounts = match &services {
            Some(services) => match Accounts::load(&services.accounts_file()) {
                Ok(accounts) => accounts,
                Err(error) => {
                    report(Err(error.to_string()));
                    Accounts::default()
                }
            },
            None => Accounts::default(),
        };
        let settings = SettingsPage::new(&preferences);
        Launcher {
            sender,
            services,
            fatal,
            page: Page::Library,
            previous: Page::Library,
            preferences,
            instances: Vec::new(),
            selected: None,
            creating: None,
            confirm: None,
            notice,
            authenticator: None,
            accounts,
            signing: None,
            account_menu: false,
            runs: Runs::default(),
            logs: Logs::default(),
            instance: InstancePage::default(),
            discover: Discover::default(),
            project: None,
            world_pick: None,
            skins: Skins::default(),
            settings,
        }
    }

    pub(crate) fn start(&mut self) {
        if self.services.is_none() {
            return;
        }
        self.reload_instances();
        spawn(
            Authenticator::open(USER_AGENT),
            |launcher: &mut Launcher, opened: launcher_auth::Result<Authenticator>| match opened {
                Ok(authenticator) => {
                    launcher.authenticator = Some(Arc::new(authenticator));
                    launcher.load_skins();
                }
                Err(error) => launcher.fail(tr!(
                    "launcher.accounts.keyring-failed",
                    reason = &error.to_string()
                )),
            },
        );
        self.start_discover();
    }

    pub(crate) fn services(&self) -> Option<Arc<Services>> {
        self.services.clone()
    }

    pub(crate) fn go(&mut self, page: Page) {
        if self.page != page {
            self.previous = self.page;
            self.page = page;
        }
        match page {
            Page::Skins => self.load_skins(),
            Page::Discover => self
                .discover
                .ensure_target(&self.instances, self.selected.as_deref()),
            Page::Library | Page::Instance | Page::Project | Page::Settings => {}
        }
    }

    pub(crate) fn back(&mut self) {
        let previous = match self.previous {
            Page::Project => Page::Discover,
            other => other,
        };
        self.page = previous;
        self.previous = Page::Library;
    }

    pub(crate) fn tell(&mut self, message: impl Into<Label>) {
        self.notice = Some(Notice {
            message: message.into().to_string(),
            failed: false,
        });
    }

    pub(crate) fn fail(&mut self, message: impl Into<Label>) {
        self.notice = Some(Notice {
            message: message.into().to_string(),
            failed: true,
        });
    }

    fn dismiss(&mut self) {
        self.notice = None;
    }

    pub(crate) fn ask(&mut self, confirm: Confirm) {
        self.confirm = Some(confirm);
    }

    fn cancel_confirm(&mut self) {
        self.confirm = None;
    }

    fn accept_confirm(&mut self) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm {
            Confirm::DeleteInstance(id) => self.delete_instance(&id),
            Confirm::DeleteWorld {
                instance, folder, ..
            } => self.delete_world(&instance, &folder),
            Confirm::RemoveContent { instance, key, .. } => self.remove_content(&instance, &key),
            Confirm::RemoveServer {
                instance, address, ..
            } => self.remove_server(&instance, &address),
            Confirm::DeleteScreenshot { path } => self.delete_screenshot(&path),
            Confirm::RemoveAccount { id, .. } => self.remove_account(&id),
        }
    }

    pub(crate) fn save_preferences(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        if let Err(error) = self.preferences.save(&services.preferences_file()) {
            self.fail(error);
        }
    }

    pub(crate) fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|instance| instance.id == id)
    }

    pub(crate) fn replace_instance(&mut self, instance: Instance) {
        match self
            .instances
            .iter_mut()
            .find(|known| known.id == instance.id)
        {
            Some(known) => *known = instance,
            None => self.instances.push(instance),
        }
    }
}

pub(crate) fn view(launcher: &Launcher) -> impl View {
    if let Some(fatal) = &launcher.fatal {
        return status_page(
            MaterialSymbol::ERROR,
            tr!("launcher.fatal", reason = fatal.as_str()),
        )
        .tint(palette().danger)
        .fill()
        .any();
    }
    let body = column! {
        notice(launcher);
        page(launcher).grow(1.0).fill_width();
    }
    .fill();
    let mut layers = vec![
        row! {
            sidebar::view(launcher);
            rule_upright();
            body.grow(1.0).fill_height();
        }
        .fill()
        .build()
        .keyed(STAGE),
    ];
    layers.extend(library::creating_sheet(launcher));
    layers.extend(accounts::sign_in_sheet(launcher));
    layers.extend(discover::world_sheet(launcher));
    layers.extend(confirm_sheet(launcher));
    column_of(layers).fill().any()
}

fn page(launcher: &Launcher) -> Element {
    let content = match launcher.page {
        Page::Library => library::view(launcher).any(),
        Page::Instance => instance::view(launcher),
        Page::Discover => discover::view(launcher),
        Page::Project => discover::project_view(launcher),
        Page::Skins => skins::view(launcher),
        Page::Settings => settings::view(launcher),
    };
    page_stack(launcher.page as usize, content).fill().any()
}

fn notice(launcher: &Launcher) -> Option<Element> {
    let notice = launcher.notice.as_ref()?;
    let (symbol, tint) = match notice.failed {
        true => (MaterialSymbol::ERROR, palette().danger),
        false => (MaterialSymbol::CHECK_CIRCLE, palette().success),
    };
    Some(
        container(
            banner(notice.message.clone())
                .symbol(symbol)
                .tint(tint)
                .on_dismiss(tr!("launcher.action.dismiss"), Launcher::dismiss),
        )
        .padding(Inset::xy(Space::LOOSE, Space::SNUG))
        .fill_width()
        .transition(fade())
        .any(),
    )
}

fn confirm_sheet(launcher: &Launcher) -> Vec<Element> {
    let Some(confirm) = &launcher.confirm else {
        return Vec::new();
    };
    let (heading, body, action) = match confirm {
        Confirm::DeleteInstance(id) => (
            tr!("launcher.confirm.delete-instance"),
            tr!(
                "launcher.confirm.delete-instance-body",
                name = launcher
                    .instance(id)
                    .map_or(id.as_str(), |instance| instance.config.name.as_str())
            ),
            tr!("launcher.action.delete"),
        ),
        Confirm::DeleteWorld { name, .. } => (
            tr!("launcher.confirm.delete-world"),
            tr!("launcher.confirm.delete-world-body", name = name.as_str()),
            tr!("launcher.action.delete"),
        ),
        Confirm::RemoveContent { title, .. } => (
            tr!("launcher.confirm.remove-content"),
            tr!(
                "launcher.confirm.remove-content-body",
                name = title.as_str()
            ),
            tr!("launcher.action.remove"),
        ),
        Confirm::RemoveServer { name, .. } => (
            tr!("launcher.confirm.remove-server"),
            tr!("launcher.confirm.remove-server-body", name = name.as_str()),
            tr!("launcher.action.remove"),
        ),
        Confirm::DeleteScreenshot { path } => (
            tr!("launcher.confirm.delete-screenshot"),
            tr!(
                "launcher.confirm.delete-screenshot-body",
                name = &path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ),
            tr!("launcher.action.delete"),
        ),
        Confirm::RemoveAccount { name, .. } => (
            tr!("launcher.confirm.remove-account"),
            tr!("launcher.confirm.remove-account-body", name = name.as_str()),
            tr!("launcher.action.remove"),
        ),
    };
    sheet(
        column! {
            text(heading).label().semibold();
            text(body).label().muted().fill_width();
            row! {
                spacer().grow(1.0);
                button(text(tr!("launcher.action.cancel")))
                    .ghost()
                    .on_press(Launcher::cancel_confirm);
                button(text(action))
                    .danger()
                    .on_press(Launcher::accept_confirm);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::ROOMY)
        .padding(20.0)
        .fill_width(),
    )
    .width(DIALOG)
    .on_dismiss(Launcher::cancel_confirm)
    .keyed("launcher-confirm")
    .centered_on(STAGE)
    .with_scrim()
}
