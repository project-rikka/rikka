use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use launcher_auth::Session;
use launcher_content::Migration;
use launcher_game::{Exit, Game, Launch, Player, QuickPlay, split_arguments};
use launcher_install::{Progress, Verify};
use launcher_instance::Instance;
use launcher_meta::{GameTarget, Platform};
use rikka_app::prelude::*;
use rikka_net::CancelToken;
use rikka_sync::Mutex;

use crate::launcher::Launcher;
use crate::logs::Pending;
use crate::preferences::Preferences;
use crate::services::{Services, scan, update};
use crate::target;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Activity {
    Idle,
    Preparing(Option<f32>),
    Running,
}

enum Run {
    Preparing {
        cancel: CancelToken,
        progress: Option<Progress>,
    },
    Running {
        game: Game,
    },
}

#[derive(Default)]
pub(crate) struct Runs {
    runs: HashMap<String, Run>,
}

impl std::fmt::Debug for Runs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Runs")
            .field("active", &self.runs.len())
            .finish()
    }
}

impl Runs {
    pub(crate) fn activity(&self, id: &str) -> Activity {
        match self.runs.get(id) {
            None => Activity::Idle,
            Some(Run::Preparing { progress, .. }) => {
                Activity::Preparing(progress.and_then(|progress| progress.fraction()))
            }
            Some(Run::Running { .. }) => Activity::Running,
        }
    }

    pub(crate) fn progress(&self, id: &str) -> Option<Progress> {
        match self.runs.get(id)? {
            Run::Preparing { progress, .. } => *progress,
            Run::Running { .. } => None,
        }
    }
}

struct Started {
    game: Game,
    session: Session,
    instance: Instance,
    migration: Option<Migration>,
}

struct Job {
    id: String,
    services: Arc<Services>,
    authenticator: Arc<launcher_auth::Authenticator>,
    account: String,
    preferences: Preferences,
    quick_play: Option<QuickPlay>,
    cancel: CancelToken,
    sender: Sender<Launcher>,
    pending: Pending,
}

fn throttled(id: String, sender: Sender<Launcher>) -> impl Fn(Progress) + Send + Sync + 'static {
    let last: Mutex<Option<(Instant, launcher_install::Stage)>> = Mutex::new(None);
    move |progress: Progress| {
        let now = Instant::now();
        let mut last = last.lock();
        let due = match *last {
            Some((at, stage)) => {
                stage != progress.stage
                    || now.duration_since(at) >= PROGRESS_INTERVAL
                    || progress.files == progress.total_files
            }
            None => true,
        };
        if !due {
            return;
        }
        *last = Some((now, progress.stage));
        let id = id.clone();
        if sender
            .send(move |launcher: &mut Launcher| launcher.runs_progress(&id, progress))
            .is_err()
        {
            tracing::debug!("install progress after the window closed");
        }
    }
}

enum Failure {
    Cancelled,
    Failed(String),
}

fn failed(error: impl std::fmt::Display) -> Failure {
    Failure::Failed(error.to_string())
}

impl Job {
    async fn migrate(&self, target: &GameTarget) -> Result<(Instance, Option<Migration>), Failure> {
        let instance = self.services.instances.get(&self.id).map_err(failed)?;
        if instance.config.game.as_ref() == Some(target) {
            return Ok((instance, None));
        }
        let mut migration = None;
        let mut content = None;
        if instance.config.game.is_some() {
            let (mut scanned, files) = scan(self.services.clone(), self.id.clone())
                .await
                .map_err(failed)?;
            migration = Some(
                self.services
                    .catalog
                    .migrate(&mut scanned, &files, &target.minecraft)
                    .await
                    .map_err(failed)?,
            );
            content = Some(scanned.config.content);
        }
        let wanted = target.clone();
        let instance = update(self.services.clone(), self.id.clone(), move |config| {
            config.game = Some(wanted);
            if let Some(content) = content {
                config.content = content;
            }
        })
        .await
        .map_err(failed)?;
        Ok((instance, migration))
    }

    async fn run(self) -> Result<Started, Failure> {
        let session = launcher_auth::session(&self.authenticator, &self.account)
            .await
            .map_err(failed)?;
        let target = target::rikka();
        let (instance, migration) = self.migrate(&target).await?;
        let installation = self
            .services
            .installer
            .install(
                &target,
                &self.preferences.java_choice(),
                Verify::Presence,
                self.cancel.clone(),
                throttled(self.id.clone(), self.sender.clone()),
            )
            .await
            .map_err(|error| match error {
                launcher_install::Error::Cancelled => Failure::Cancelled,
                error => failed(error),
            })?;
        let arguments = instance
            .config
            .jvm_arguments
            .as_deref()
            .unwrap_or(&self.preferences.jvm_arguments);
        let arguments = split_arguments(arguments).map_err(failed)?;
        let player = Player {
            name: session.name.clone(),
            id: session.id.clone(),
            xuid: session.xuid.clone(),
            access_token: session.access_token().to_owned(),
        };
        let mut launch = Launch::new(installation, &instance.path, player)
            .memory(
                instance
                    .config
                    .memory_mb
                    .unwrap_or_else(|| self.preferences.memory()),
            )
            .jvm_arguments(arguments)
            .quick_play(self.quick_play)
            .launcher_version(env!("CARGO_PKG_VERSION"));
        if let Some(window) = instance.config.window.or(self.preferences.window) {
            launch = launch
                .window(window.width, window.height)
                .fullscreen(window.fullscreen);
        }
        let command = launch.command(Platform::current()).map_err(failed)?;
        if self.cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let lines_id = self.id.clone();
        let lines_sender = self.sender.clone();
        let pending = self.pending.clone();
        let exit_id = self.id.clone();
        let exit_sender = self.sender.clone();
        let game = Game::spawn(
            command,
            move |line| pending.push(&lines_id, line, &lines_sender),
            move |exit| {
                if exit_sender
                    .send(move |launcher: &mut Launcher| launcher.exited(&exit_id, exit))
                    .is_err()
                {
                    tracing::debug!("the game exited after the window closed");
                }
            },
        )
        .map_err(failed)?;
        Ok(Started {
            game,
            session,
            instance,
            migration,
        })
    }
}

impl Launcher {
    pub(crate) fn play(&mut self, id: String, quick_play: Option<QuickPlay>) {
        if self.runs.runs.contains_key(&id) {
            return;
        }
        let Some(services) = self.services() else {
            return;
        };
        let Some(authenticator) = self.authenticator.clone() else {
            self.fail(tr!("launcher.accounts.keyring-wait"));
            return;
        };
        let Some(account) = self.accounts.active().map(|account| account.id.clone()) else {
            self.fail(tr!("launcher.accounts.needed"));
            self.begin_sign_in();
            return;
        };
        let cancel = CancelToken::new();
        self.runs.runs.insert(
            id.clone(),
            Run::Preparing {
                cancel: cancel.clone(),
                progress: None,
            },
        );
        self.logs.start(&id);
        let job = Job {
            id: id.clone(),
            services,
            authenticator,
            account,
            preferences: self.preferences.clone(),
            quick_play,
            cancel,
            sender: self.sender.clone(),
            pending: Pending::default(),
        };
        spawn_keyed(
            ("launcher-run", id.clone()),
            job.run(),
            move |launcher: &mut Launcher, started: Result<Started, Failure>| {
                launcher.started(&id, started);
            },
        );
    }

    fn runs_progress(&mut self, id: &str, shown: Progress) {
        if let Some(Run::Preparing { progress, .. }) = self.runs.runs.get_mut(id) {
            *progress = Some(shown);
        }
    }

    fn started(&mut self, id: &str, started: Result<Started, Failure>) {
        let still_preparing = matches!(self.runs.runs.get(id), Some(Run::Preparing { .. }));
        match started {
            Ok(started) => {
                self.accounts.remember(&started.session);
                self.save_accounts();
                self.replace_instance(started.instance);
                if let Some(migration) = started.migration
                    && !migration.incompatible.is_empty()
                {
                    self.fail(tr!(
                        "launcher.run.migrated",
                        count = &migration.incompatible.len().to_string()
                    ));
                }
                match still_preparing {
                    true if started.game.is_running() => {
                        self.runs
                            .runs
                            .insert(id.to_owned(), Run::Running { game: started.game });
                    }
                    true => {}
                    false => {
                        if let Err(error) = started.game.stop() {
                            self.fail(error.to_string());
                        }
                    }
                }
            }
            Err(failure) => {
                self.runs.runs.remove(id);
                if let Failure::Failed(reason) = failure {
                    self.fail(tr!("launcher.run.failed", reason = reason.as_str()));
                }
            }
        }
    }

    fn exited(&mut self, id: &str, exit: Exit) {
        self.runs.runs.remove(id);
        if exit.crashed() {
            self.fail(tr!(
                "launcher.run.crashed",
                code = &exit.code.unwrap_or_default().to_string()
            ));
        }
        let Some(services) = self.services() else {
            return;
        };
        let ended = jiff::Timestamp::now().as_second();
        spawn(
            update(services, id.to_owned(), move |config| {
                config.add_play_time(exit.played.as_secs(), ended);
            }),
            |launcher: &mut Launcher, saved: launcher_instance::Result<Instance>| match saved {
                Ok(instance) => launcher.replace_instance(instance),
                Err(error) => launcher.fail(error.to_string()),
            },
        );
    }

    pub(crate) fn stop(&mut self, id: &str) {
        match self.runs.runs.get(id) {
            Some(Run::Preparing { cancel, .. }) => {
                cancel.cancel();
                self.runs.runs.remove(id);
            }
            Some(Run::Running { game }) => {
                if let Err(error) = game.stop() {
                    self.fail(error.to_string());
                }
            }
            None => {}
        }
    }
}
