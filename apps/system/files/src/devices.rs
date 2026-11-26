use std::rc::Rc;
use std::sync::Arc;

use files_config::Config;
use files_core::Location;
use files_drives::{Drives, Error, Volume, VolumeId, VolumeKind};
use rikka_app::prelude::*;

use crate::services::{Services, SharedServices};

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Mount { id: VolumeId, open: bool },
    Unmount(VolumeId),
    Eject(VolumeId),
    Unlock { id: VolumeId, passphrase: String },
    Lock(VolumeId),
}

impl Command {
    pub(crate) fn id(&self) -> &VolumeId {
        match self {
            Self::Mount { id, .. }
            | Self::Unmount(id)
            | Self::Eject(id)
            | Self::Unlock { id, .. }
            | Self::Lock(id) => id,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Outcome {
    pub(crate) id: VolumeId,
    pub(crate) failure: Option<Error>,
    pub(crate) open: Option<Location>,
    pub(crate) unlocked: Option<VolumeId>,
}

impl Outcome {
    fn new(id: &VolumeId) -> Self {
        Self {
            id: id.clone(),
            failure: None,
            open: None,
            unlocked: None,
        }
    }
}

#[derive(State)]
pub(crate) struct Devices {
    #[untracked]
    places: Rc<SharedServices>,
    #[untracked]
    automount: Option<Config>,
    #[untracked]
    finished: Handler<(Outcome,)>,
}

impl Devices {
    pub(crate) fn new<M>(
        services: &Services,
        on_finished: impl IntoHandler<(Outcome,), (), M>,
    ) -> Self {
        let on_finished = on_finished.into_handler();
        Self {
            places: services.shared(),
            automount: None,
            finished: Handler::relaying(move |reach, (outcome,)| {
                let outcome = reach.get::<Devices>()?.finished(outcome);
                on_finished.call(reach, (outcome,))
            }),
        }
    }

    #[must_use]
    pub(crate) fn automounting(mut self) -> Self {
        self.automount = Some(Config::load());
        self
    }

    pub(crate) fn look(&mut self) {
        let drives = Arc::clone(&self.places.drives);
        spawn_blocking(move || drives.snapshot(), Devices::volumes);
    }

    pub(crate) fn run(&mut self, command: Command) {
        self.places.working(command.id(), true);
        let drives = Arc::clone(&self.places.drives);
        spawn_blocking(
            move || run_command(&drives, &command),
            self.finished.clone(),
        );
    }

    fn volumes(&mut self, volumes: Vec<Volume>) {
        let arrived = automountable(&volumes);
        self.places.adopt(volumes);
        if !self.automount.as_mut().is_some_and(Config::automount) {
            return;
        }
        for id in self.places.claim_automount(arrived) {
            self.run(Command::Mount { id, open: false });
        }
    }

    fn finished(&mut self, mut outcome: Outcome) -> Outcome {
        self.places.working(&outcome.id, false);
        self.look();
        if let Some(id) = outcome.unlocked.take() {
            self.run(Command::Mount { id, open: true });
        }
        outcome
    }
}

impl std::fmt::Debug for Devices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Devices")
            .field("automount", &self.automount.is_some())
            .finish_non_exhaustive()
    }
}

fn run_command(drives: &Drives, command: &Command) -> Outcome {
    let mut outcome = Outcome::new(command.id());
    match command {
        Command::Mount { id, open } => match drives.mount(id, *open) {
            Ok(at) => outcome.open = open.then(|| Location::from_path(at)),
            Err(error) => outcome.failure = Some(error),
        },
        Command::Unmount(id) => outcome.failure = drives.unmount(id).err(),
        Command::Eject(id) => outcome.failure = drives.eject(id).err(),
        Command::Lock(id) => outcome.failure = drives.lock(id).err(),
        Command::Unlock { id, passphrase } => match drives.unlock(id, passphrase) {
            Ok(opened) => outcome.unlocked = Some(opened),
            Err(error) => outcome.failure = Some(error),
        },
    }
    outcome
}

fn automountable(volumes: &[Volume]) -> Vec<VolumeId> {
    volumes
        .iter()
        .filter(|volume| volume.kind != VolumeKind::Internal)
        .filter(|volume| volume.actions.mount && !volume.locked)
        .map(|volume| volume.id.clone())
        .collect()
}
