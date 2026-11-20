use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::Stream;

use proton_config::Config;
use rikka_app::prelude::*;
use rikka_proton_cache::Cache;
use rikka_proton_client::Session;

use crate::account::Account;
use crate::clients::Clients;
use crate::codes::Codes;
use crate::desk::Desk;
use crate::drive::Drive;
use crate::mail::Mail;
use crate::pass::Pass;
use crate::product::Product;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceId(u64);

#[derive(Debug, State)]
pub(crate) struct Workspace {
    workspace_id: WorkspaceId,
    clients: Clients,
    #[state]
    pub(crate) mail: Mail,
    #[state]
    pub(crate) drive: Drive,
    #[state]
    pub(crate) pass: Pass,
    #[state]
    pub(crate) codes: Codes,
    #[state]
    pub(crate) account: Account,
}

impl Workspace {
    pub(crate) const COMMANDS: [Command; 6] = [
        Command::Refresh,
        Command::Find,
        Command::Delete,
        Command::Back,
        Command::Up,
        Command::Rename,
    ];

    pub(crate) fn new(session: Session, cache: Cache, config: &Config) -> Workspace {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let workspace_id = WorkspaceId(NEXT.fetch_add(1, Ordering::Relaxed));
        let clients = Clients::new(session);

        Workspace {
            mail: Mail::new(clients.clone(), workspace_id, config),
            drive: Drive::new(clients.clone(), cache.clone(), workspace_id, config),
            pass: Pass::new(clients.clone(), workspace_id),
            codes: Codes::new(clients.clone(), cache, workspace_id),
            account: Account::new(clients.session().clone()),
            workspace_id,
            clients,
        }
    }

    pub(crate) fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub(crate) fn clients(&self) -> &Clients {
        &self.clients
    }

    pub(crate) fn session(&self) -> &Session {
        self.clients.session()
    }

    pub(crate) fn describe(&mut self, session: Session) {
        self.clients = self.clients.renewed(session);
        self.mail.describe(self.clients.clone());
        self.drive.describe(self.clients.clone());
        self.pass.describe(self.clients.clone());
        self.codes.describe(self.clients.clone());
        self.account.describe(self.clients.session().clone());
    }

    pub(crate) fn command(&mut self, product: Product, command: Command) {
        match (product, command) {
            (Product::Mail, Command::Refresh) => self.mail.refresh(),
            (Product::Mail, Command::Find) => self.mail.seek(),
            (Product::Mail, Command::Delete) => self.mail.throw(),
            (Product::Drive, Command::Refresh) => self.drive.refresh(),
            (Product::Drive, Command::Back | Command::Up) => self.drive.up(),
            (Product::Drive, Command::Delete) => self.drive.throw(),
            (Product::Drive, Command::Rename) => self.drive.start_rename(),
            (Product::Pass, Command::Refresh) => self.pass.refresh(),
            (Product::Pass, Command::Find) => self.pass.seek(),
            (Product::Pass, Command::Delete) => self.pass.throw(),
            (Product::Codes, Command::Refresh) => self.codes.sync(),
            (Product::Codes, Command::Find) => self.codes.seek(),
            _ => skip_view(),
        }
    }

    pub(crate) fn opened(&mut self, product: Product) {
        match product {
            Product::Mail => self.mail.opened(),
            Product::Drive => self.drive.opened(),
            Product::Pass => self.pass.opened(),
            Product::Codes => self.codes.opened(),
            Product::Account => self.account.opened(),
        }
    }

    pub(crate) fn rest(&mut self, product: Product) {
        match product {
            Product::Pass => self.pass.rest(),
            Product::Codes => self.codes.rest(),
            Product::Mail | Product::Drive | Product::Account => {}
        }
    }
}

pub(crate) trait ProductState: Sized + 'static {
    fn from_workspace(workspace: &mut Workspace) -> &mut Self;

    fn workspace_id(&self) -> WorkspaceId;

    fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
        apply: impl Fn(&mut Self, T) + 'static,
    ) {
        spawn(future, landing(self.workspace_id(), apply));
    }

    fn spawn_keyed<T: Send + 'static>(
        &self,
        key: impl Hash,
        future: impl Future<Output = T> + Send + 'static,
        apply: impl Fn(&mut Self, T) + 'static,
    ) {
        spawn_keyed(key, future, landing(self.workspace_id(), apply));
    }

    fn spawn_blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
        apply: impl Fn(&mut Self, T) + 'static,
    ) {
        spawn_blocking(work, landing(self.workspace_id(), apply));
    }

    fn stream<T: Send + 'static>(
        &self,
        items: impl Stream<Item = T> + Send + 'static,
        apply: impl Fn(&mut Self, T) + 'static,
    ) {
        stream(items, landing(self.workspace_id(), apply));
    }

    fn stream_keyed<T: Send + 'static>(
        &self,
        key: impl Hash,
        items: impl Stream<Item = T> + Send + 'static,
        apply: impl Fn(&mut Self, T) + 'static,
    ) {
        stream_keyed(key, items, landing(self.workspace_id(), apply));
    }

    fn after_keyed(&self, key: impl Hash, delay: Duration, apply: impl Fn(&mut Self) + 'static) {
        after_keyed(key, delay, ticking(self.workspace_id(), apply));
    }

    fn every_keyed(&self, key: impl Hash, period: Duration, apply: impl Fn(&mut Self) + 'static) {
        every_keyed(key, period, ticking(self.workspace_id(), apply));
    }
}

pub(crate) fn landing<P: ProductState, T: 'static>(
    workspace_id: WorkspaceId,
    apply: impl Fn(&mut P, T) + 'static,
) -> impl Fn(&mut Desk, T) + 'static {
    move |desk: &mut Desk, value: T| match desk.workspace_of(workspace_id) {
        Some(workspace) => apply(P::from_workspace(workspace), value),
        None => skip_view(),
    }
}

fn ticking<P: ProductState>(
    workspace_id: WorkspaceId,
    apply: impl Fn(&mut P) + 'static,
) -> impl Fn(&mut Desk) + 'static {
    move |desk: &mut Desk| match desk.workspace_of(workspace_id) {
        Some(workspace) => apply(P::from_workspace(workspace)),
        None => skip_view(),
    }
}
