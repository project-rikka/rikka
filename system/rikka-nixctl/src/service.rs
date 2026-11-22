use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_channel::mpsc::UnboundedSender;
use nixctl_task::{Directory, Site};
use rikka_nix::{Document, Module};
use rikka_nix_journal::{Journal, Transcript};
use rikka_nix_protocol::{
    Action, ModuleFile, Operation, PageRecord, Request, RunId, RunRecord, StatusRecord,
};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, DBusError, interface};

use crate::polkit;
use crate::run_guard::RunGuard;

const MODULE_BYTES: usize = 1024 * 1024;

#[derive(Debug, DBusError)]
#[zbus(prefix = "os.rikka.Nix1")]
pub(crate) enum Error {
    Busy(String),
    Denied(String),
    Stale(String),
    Invalid(String),
    Unmanaged(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Shared(Arc<Mutex<Journal>>);

impl Shared {
    pub(crate) fn new(journal: Journal) -> Self {
        Self(Arc::new(Mutex::new(journal)))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Journal> {
        self.0.lock()
    }
}

#[derive(Debug)]
pub(crate) struct Accepted {
    pub(crate) request: Request,
    pub(crate) site: Site,
    pub(crate) run: RunId,
    pub(crate) transcript: Transcript,
    pub(crate) guard: RunGuard,
}

#[derive(Debug)]
pub(crate) struct Nix {
    running: Arc<AtomicBool>,
    requests: UnboundedSender<Accepted>,
    journal: Shared,
}

#[interface(name = "os.rikka.Nix1")]
impl Nix {
    async fn apply(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        files: Vec<(String, String, String)>,
        action: String,
    ) -> Result<u64, Error> {
        let request = Request::Apply {
            files: named(files)?,
            action: parse_action(&action)?,
        };
        self.begin(&header, connection, &emitter, request).await
    }

    async fn rollback(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        generation: u32,
    ) -> Result<u64, Error> {
        self.begin(
            &header,
            connection,
            &emitter,
            Request::Rollback { generation },
        )
        .await
    }

    async fn update(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        action: String,
    ) -> Result<u64, Error> {
        let request = Request::Update {
            action: parse_action(&action)?,
        };
        self.begin(&header, connection, &emitter, request).await
    }

    async fn runs(&self) -> Vec<RunRecord> {
        self.journal.lock().runs().map(RunRecord::from).collect()
    }

    async fn transcript(&self, run: u64, from: u64) -> Result<PageRecord, Error> {
        self.journal
            .lock()
            .page(RunId::new(run), from)
            .map_err(|error| match error {
                rikka_nix_journal::Error::UnknownRun(_)
                | rikka_nix_journal::Error::Offset { .. } => Error::Invalid(error.to_string()),
                other => Error::Failed(other.to_string()),
            })
    }

    #[zbus(signal)]
    pub(crate) async fn started(emitter: &SignalEmitter<'_>, run: RunRecord) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn lines(
        emitter: &SignalEmitter<'_>,
        run: u64,
        from: u64,
        lines: &[String],
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn status(
        emitter: &SignalEmitter<'_>,
        run: u64,
        status: StatusRecord,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn finished(emitter: &SignalEmitter<'_>, run: RunRecord) -> zbus::Result<()>;
}

impl Nix {
    pub(crate) fn new(
        requests: UnboundedSender<Accepted>,
        running: Arc<AtomicBool>,
        journal: Shared,
    ) -> Self {
        Self {
            running,
            requests,
            journal,
        }
    }

    async fn begin(
        &self,
        header: &Header<'_>,
        connection: &Connection,
        emitter: &SignalEmitter<'_>,
        request: Request,
    ) -> Result<u64, Error> {
        if self.running.load(Ordering::Acquire) {
            return Err(busy());
        }
        let site = Site::system().map_err(|error| Error::Unmanaged(error.to_string()))?;
        self.authorize(header, connection).await?;
        let guard = RunGuard::try_acquire(&self.running).ok_or_else(busy)?;
        admit(&request, &site)?;
        let (run, transcript) = self
            .journal
            .lock()
            .begin(Operation::from(&request))
            .map_err(|error| Error::Failed(error.to_string()))?;
        if let Err(error) = Self::started(emitter, RunRecord::from(&run)).await {
            eprintln!("failed to emit Started: {error}");
        }
        self.requests
            .unbounded_send(Accepted {
                request,
                site,
                run: run.id,
                transcript,
                guard,
            })
            .map_err(|_| Error::Busy("rikka-nixctl is shutting down".to_owned()))?;
        Ok(run.id.get())
    }

    async fn authorize(&self, header: &Header<'_>, connection: &Connection) -> Result<(), Error> {
        let Some(caller) = header.sender() else {
            return Err(Error::Denied("caller has no unique bus name".to_owned()));
        };
        match polkit::is_authorized(connection, caller).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Denied("authorization denied".to_owned())),
            Err(reason) => Err(Error::Denied(reason.to_string())),
        }
    }
}

fn busy() -> Error {
    Error::Busy("another change is already being applied".to_owned())
}

fn named(files: Vec<(String, String, String)>) -> Result<Vec<ModuleFile>, Error> {
    let mut named: Vec<ModuleFile> = Vec::with_capacity(files.len());
    for (module, base, contents) in files {
        let module = match Module::from_name(&module) {
            Some(Module::Root) => {
                return Err(Error::Invalid(format!(
                    "{} is written by rikka-nixctl itself",
                    Module::Root.file_name()
                )));
            }
            Some(module) => module,
            None => return Err(Error::Invalid(format!("unknown module `{module}`"))),
        };
        if named.iter().any(|file| file.module == module) {
            return Err(Error::Invalid(format!(
                "{} is listed twice",
                module.file_name()
            )));
        }
        if base.len().max(contents.len()) > MODULE_BYTES {
            return Err(Error::Invalid(format!(
                "{} is larger than {MODULE_BYTES} bytes",
                module.file_name()
            )));
        }
        named.push(ModuleFile {
            module,
            base,
            contents,
        });
    }
    Ok(named)
}

fn admit(request: &Request, site: &Site) -> Result<(), Error> {
    let Request::Apply { files, .. } = request else {
        return Ok(());
    };
    for file in files {
        Document::parse(file.contents.as_str())
            .map_err(|error| Error::Invalid(format!("{}: {error}", file.module.file_name())))?;
    }
    let directory = Directory::open(&site.modules).map_err(|error| {
        Error::Failed(format!(
            "failed to read {}: {error}",
            site.modules.display()
        ))
    })?;
    match directory.stale(files) {
        Some(module) => Err(Error::Stale(format!(
            "{} changed since it was read",
            module.file_name()
        ))),
        None => Ok(()),
    }
}

fn parse_action(action: &str) -> Result<Action, Error> {
    Action::from_name(action)
        .ok_or_else(|| Error::Invalid(format!("unknown rebuild action `{action}`")))
}
