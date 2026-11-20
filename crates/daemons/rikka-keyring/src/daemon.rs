use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;
use rikka_secrets_protocol::names::BUS_NAME;
use zbus::Connection;
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};

use crate::error::{Error, Result};
use crate::shared::Shared;
use crate::state::State;

const OWNER_WATCH: &str = "rikka-keyring-owners";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Binding {
    #[default]
    None,
    SystemdCredentials,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Daemon {
    folder: PathBuf,
    binding: Binding,
}

#[derive(Debug)]
pub struct Served {
    connection: Connection,
}

impl Daemon {
    pub fn new(folder: impl Into<PathBuf>) -> Daemon {
        Daemon {
            folder: folder.into(),
            binding: Binding::None,
        }
    }

    pub fn binding(mut self, binding: Binding) -> Daemon {
        self.binding = binding;
        self
    }

    pub async fn serve(self) -> Result<Served> {
        let Daemon { folder, binding } = self;
        let state = blocking::unblock(move || State::load(folder, binding)).await?;
        let shared = Shared::new(state);
        let connection = zbus::connection::Builder::session()?.build().await?;
        shared.attach(connection.clone());
        shared.publish_all().await?;
        watch_owners(&connection, &shared).await?;
        let reply = connection
            .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
            .await?;
        if reply != RequestNameReply::PrimaryOwner {
            return Err(Error::Taken);
        }
        tracing::info!("serving {BUS_NAME}");
        Ok(Served { connection })
    }
}

impl Served {
    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

async fn watch_owners(connection: &Connection, shared: &Arc<Shared>) -> Result<()> {
    let mut changes = DBusProxy::new(connection)
        .await?
        .receive_name_owner_changed()
        .await?;
    let shared = Arc::clone(shared);
    connection
        .executor()
        .spawn(
            async move {
                while let Some(change) = changes.next().await {
                    let Ok(args) = change.args() else {
                        continue;
                    };
                    if args.new_owner().is_none() && args.name().starts_with(':') {
                        shared.client_left(args.name().to_string()).await;
                    }
                }
            },
            OWNER_WATCH,
        )
        .detach();
    Ok(())
}
