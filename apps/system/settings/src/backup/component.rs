use std::path::PathBuf;

use rikka_app::prelude::*;
use rikka_backup::{Bundle, EXTENSION};
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_seam::SeamClient;
use rikka_settings::seam::CAPABILITY;
use settings_backup::{Config, Published};
use zeroize::Zeroizing;

use crate::backup::work::{self, Account, Documents, Exported, Restored};
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Work {
    Exporting,
    Opening,
    Restoring,
    Publishing,
    Fetching,
}

#[derive(Debug)]
pub(crate) enum Report {
    Exported { path: PathBuf, moved: usize },
    Restored { staged: usize, skipped: Vec<String> },
    Published(Published),
    Fetched { staged: usize },
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Opened {
    pub(crate) path: PathBuf,
    pub(crate) bundle: Bundle,
}

struct Unlocking {
    unlocked: Result<(), String>,
    account: Account,
    opened: Opened,
}

pub(crate) struct Backup {
    pub(crate) account: Result<Account, String>,
    seam: Option<SeamClient>,
    config: Option<Config>,
    pub(crate) passphrase: TextDocument,
    pub(crate) confirm: TextDocument,
    pub(crate) unlock: TextDocument,
    pub(crate) repository: TextDocument,
    pub(crate) revealed: bool,
    pub(crate) sign_ins: bool,
    pub(crate) chosen: Option<PathBuf>,
    pub(crate) opened: Option<Opened>,
    pub(crate) work: Option<Work>,
    pub(crate) report: Option<Report>,
}

impl Page for Backup {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.account = Account::current();
        self.seam = session.seam(CAPABILITY).ok();
        let config = self.config.get_or_insert_with(Config::load);
        self.repository.reset(config.repository());
        if self.chosen.is_none()
            && let Ok(account) = &self.account
            && account.dirs.pending().is_file()
        {
            self.chosen = Some(account.dirs.pending());
        }
    }

    fn closed(&mut self) {
        for document in [&self.passphrase, &self.confirm, &self.unlock] {
            document.reset("");
        }
    }
}

impl Backup {
    pub(crate) fn new() -> Backup {
        Backup {
            account: Err(String::new()),
            seam: None,
            config: None,
            passphrase: TextDocument::new(""),
            confirm: TextDocument::new(""),
            unlock: TextDocument::new(""),
            repository: TextDocument::new(""),
            revealed: false,
            sign_ins: true,
            chosen: None,
            opened: None,
            work: None,
            report: None,
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.work.is_some()
    }

    pub(crate) fn passphrase_ready(&self) -> bool {
        !self.passphrase.is_empty()
            && *Zeroizing::new(self.passphrase.text()) == *Zeroizing::new(self.confirm.text())
    }

    pub(crate) fn reveal(&mut self) {
        self.revealed = !self.revealed;
    }

    pub(crate) fn include_sign_ins(&mut self, on: bool) {
        self.sign_ins = on;
    }

    pub(crate) fn save(&mut self) {
        if self.is_busy() || !self.passphrase_ready() || self.account.is_err() {
            return;
        }
        self.work = Some(Work::Exporting);
        self.report = None;
        spawn(
            unlock(self.sign_ins),
            |backup: &mut Backup, unlocked: Result<(), String>| match unlocked {
                Ok(()) => backup.export(),
                Err(reason) => {
                    backup.work = None;
                    backup.report = Some(Report::Failed(reason));
                }
            },
        );
    }

    fn export(&mut self) {
        let Ok(account) = self.account.clone() else {
            return;
        };
        let passphrase = Zeroizing::new(self.passphrase.text());
        let sign_ins = self.sign_ins;
        let chooser = Chooser::save(tr!("settings.backup.save-title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("settings.backup.save-accept").to_string())
            .current_name(default_name())
            .filter(backups());
        spawn_blocking(
            move || match chooser.run() {
                Ok(ChooserResponse::Files(files)) => files
                    .first()
                    .map(|target| work::export(&account, target, &passphrase, sign_ins)),
                Ok(ChooserResponse::Cancelled) => None,
                Err(error) => Some(Err(error.to_string())),
            },
            |backup: &mut Backup, exported: Option<Result<Exported, String>>| {
                backup.work = None;
                match exported {
                    None => {}
                    Some(Ok(exported)) => {
                        backup.passphrase.reset("");
                        backup.confirm.reset("");
                        backup.report = Some(Report::Exported {
                            path: exported.path,
                            moved: exported.moved,
                        });
                    }
                    Some(Err(reason)) => backup.report = Some(Report::Failed(reason)),
                }
            },
        );
    }

    pub(crate) fn choose(&mut self) {
        if self.is_busy() {
            return;
        }
        let chooser = Chooser::open(tr!("settings.backup.open-title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("settings.backup.open-accept").to_string())
            .filter(backups());
        spawn_blocking(
            move || match chooser.run() {
                Ok(ChooserResponse::Files(files)) => files.first().cloned().map(Ok),
                Ok(ChooserResponse::Cancelled) => None,
                Err(error) => Some(Err(error.to_string())),
            },
            |backup: &mut Backup, chosen: Option<Result<PathBuf, String>>| match chosen {
                None => {}
                Some(Ok(path)) => {
                    backup.chosen = Some(path);
                    backup.opened = None;
                    backup.report = None;
                }
                Some(Err(reason)) => backup.report = Some(Report::Failed(reason)),
            },
        );
    }

    pub(crate) fn open(&mut self) {
        let Some(path) = self.chosen.clone() else {
            return;
        };
        if self.is_busy() || self.unlock.is_empty() {
            return;
        }
        let passphrase = Zeroizing::new(self.unlock.text());
        self.work = Some(Work::Opening);
        self.report = None;
        spawn_blocking(
            move || work::open(&path, &passphrase).map(|bundle| Opened { path, bundle }),
            |backup: &mut Backup, opened: Result<Opened, String>| {
                backup.work = None;
                match opened {
                    Ok(opened) => {
                        backup.unlock.reset("");
                        backup.opened = Some(opened);
                    }
                    Err(reason) => backup.report = Some(Report::Failed(reason)),
                }
            },
        );
    }

    pub(crate) fn forget_opened(&mut self) {
        self.opened = None;
        self.chosen = None;
    }

    pub(crate) fn restore(&mut self) {
        let (Ok(account), Some(opened)) = (self.account.clone(), self.opened.clone()) else {
            return;
        };
        if self.is_busy() {
            return;
        }
        self.work = Some(Work::Restoring);
        self.report = None;
        let needs_keyring = !opened.bundle.credentials.is_empty();
        spawn(
            async move {
                Unlocking {
                    unlocked: unlock(needs_keyring).await,
                    account,
                    opened,
                }
            },
            |backup: &mut Backup, unlocking: Unlocking| backup.unlocked(unlocking),
        );
    }

    fn unlocked(&mut self, unlocking: Unlocking) {
        let Unlocking {
            unlocked,
            account,
            opened,
        } = unlocking;
        if let Err(reason) = unlocked {
            self.work = None;
            self.report = Some(Report::Failed(reason));
            return;
        }
        let seam = self.seam.clone();
        spawn_blocking(
            move || work::restore(&account, seam.as_ref(), &opened.bundle, &opened.path),
            |settings: &mut crate::settings::Settings, restored: Result<Restored, String>| {
                settings.backup_restored(restored);
            },
        );
    }

    pub(crate) fn restored(&mut self, pending: &mut Pending, restored: Result<Restored, String>) {
        self.work = None;
        self.report = Some(match restored {
            Ok(restored) => {
                self.opened = None;
                self.chosen = None;
                Report::Restored {
                    staged: stage(pending, restored.documents),
                    skipped: restored.skipped,
                }
            }
            Err(reason) => Report::Failed(reason),
        });
    }

    pub(crate) fn remember_repository(&mut self) {
        let url = self.repository.text();
        if let Some(config) = &mut self.config {
            config.set_repository(url.trim());
        }
    }

    pub(crate) fn publish(&mut self) {
        let Ok(account) = self.account.clone() else {
            return;
        };
        let url = self.repository.text().trim().to_owned();
        if self.is_busy() || url.is_empty() {
            return;
        }
        self.remember_repository();
        self.work = Some(Work::Publishing);
        self.report = None;
        spawn_blocking(
            move || work::publish(&account, &url),
            |backup: &mut Backup, published: Result<Published, String>| {
                backup.work = None;
                backup.report = Some(match published {
                    Ok(published) => Report::Published(published),
                    Err(reason) => Report::Failed(reason),
                });
            },
        );
    }

    pub(crate) fn fetch(&mut self) {
        let Ok(account) = self.account.clone() else {
            return;
        };
        let url = self.repository.text().trim().to_owned();
        if self.is_busy() || url.is_empty() {
            return;
        }
        self.remember_repository();
        self.work = Some(Work::Fetching);
        self.report = None;
        spawn_blocking(
            move || work::fetch(&account, &url),
            |settings: &mut crate::settings::Settings, fetched: Result<Documents, String>| {
                settings.repository_fetched(fetched);
            },
        );
    }

    pub(crate) fn fetched(&mut self, pending: &mut Pending, fetched: Result<Documents, String>) {
        self.work = None;
        self.report = Some(match fetched {
            Ok(documents) => Report::Fetched {
                staged: stage(pending, documents),
            },
            Err(reason) => Report::Failed(reason),
        });
    }
}

impl std::fmt::Debug for Backup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Backup")
            .field("work", &self.work)
            .field("chosen", &self.chosen)
            .finish_non_exhaustive()
    }
}

fn stage(pending: &mut Pending, documents: Documents) -> usize {
    let staged = documents.len();
    for (module, document) in documents {
        pending.change(module, move |current| {
            *current = document;
            Ok(())
        });
    }
    staged
}

async fn unlock(needed: bool) -> Result<(), String> {
    if !needed {
        return Ok(());
    }
    rikka_secrets::Secrets::unlocked()
        .await
        .map(drop)
        .map_err(|error| error.to_string())
}

fn backups() -> Filter {
    Filter::named(tr!("settings.backup.files").to_string()).pattern(format!("*.{EXTENSION}"))
}

fn default_name() -> String {
    let host = work::host_name().unwrap_or_else(|_| "rikka".to_owned());
    let date = jiff::Zoned::now().date();
    format!("{host}-{date}.{EXTENSION}")
}
