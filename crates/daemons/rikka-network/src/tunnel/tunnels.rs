use std::collections::HashMap;
use std::future::ready;
use std::io::ErrorKind;
use std::path::PathBuf;

use futures_util::{Stream, StreamExt, stream};
use zbus::Connection;
use zbus::zvariant::ObjectPath;

use crate::active::{ActiveId, ActiveState};
use crate::error::Result;
use crate::session::proxies::{
    ActiveConnectionProxy, ManagerProxy, SavedConnectionProxy, SettingsStoreProxy,
};
use crate::session::user;
use crate::tunnel::kind::Tunnel;
use crate::tunnel::openvpn::OpenVpnTunnel;
use crate::tunnel::state::{SavedTunnel, TunnelState};
use crate::values::Draft;

const IN_MEMORY: u32 = 0x2;
const BLOCK_AUTOCONNECT: u32 = 0x20;
const ANYWHERE: &str = "/";
const NOT_ACTIVE: &str = "org.freedesktop.NetworkManager.ConnectionNotActive";
const UNKNOWN_CONNECTION: &str = "org.freedesktop.NetworkManager.UnknownConnection";
const UNKNOWN_OBJECT: &str = "org.freedesktop.DBus.Error.UnknownObject";
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";

#[derive(Clone, Debug)]
pub struct Tunnels {
    connection: Connection,
    user: String,
}

impl Tunnels {
    pub async fn system() -> Result<Tunnels> {
        Ok(Tunnels {
            connection: Connection::system().await?,
            user: user::name(),
        })
    }

    pub async fn add(&self, tunnel: impl Into<Tunnel>) -> Result<SavedTunnel> {
        let (draft, files) = match tunnel.into() {
            Tunnel::WireGuard(tunnel) => (tunnel.draft(&self.user), Vec::new()),
            Tunnel::Protun(tunnel) => (tunnel.draft(&self.user), Vec::new()),
            Tunnel::OpenVpn(tunnel) => {
                let blobs = tunnel.blobs()?;
                (tunnel.draft(&self.user, &blobs), blobs.into_files())
            }
        };
        let added = match draft {
            Ok(draft) => self.added(&draft).await,
            Err(error) => Err(error),
        };
        match added {
            Ok(path) => Ok(SavedTunnel { path, files }),
            Err(error) => {
                forget(&files);
                Err(error)
            }
        }
    }

    async fn added(&self, draft: &Draft) -> Result<String> {
        let (path, _) = SettingsStoreProxy::new(&self.connection)
            .await?
            .add_connection2(draft, IN_MEMORY | BLOCK_AUTOCONNECT, HashMap::new())
            .await?;
        Ok(path.as_str().to_owned())
    }

    pub fn rewrite_files(&self, tunnel: &OpenVpnTunnel) -> Result<()> {
        tunnel.blobs().map(drop)
    }

    pub async fn activate(&self, tunnel: &SavedTunnel) -> Result<ActiveId> {
        let active = ManagerProxy::new(&self.connection)
            .await?
            .activate_connection(&path(tunnel.as_str())?, &path(ANYWHERE)?, &path(ANYWHERE)?)
            .await?;
        Ok(ActiveId(active.as_str().to_owned()))
    }

    pub async fn deactivate(&self, active: &ActiveId) -> Result<()> {
        let manager = ManagerProxy::new(&self.connection).await?;
        match manager.deactivate_connection(&path(active.as_str())?).await {
            Err(error) if names(&error, &[NOT_ACTIVE, UNKNOWN_CONNECTION, UNKNOWN_OBJECT]) => {
                Ok(())
            }
            answered => Ok(answered?),
        }
    }

    pub async fn remove(&self, tunnel: &SavedTunnel) -> Result<()> {
        let saved = SavedConnectionProxy::builder(&self.connection)
            .path(tunnel.as_str().to_owned())?
            .build()
            .await?;
        match saved.delete().await {
            Err(error)
                if !names(
                    &error,
                    &[UNKNOWN_CONNECTION, UNKNOWN_OBJECT, UNKNOWN_METHOD],
                ) =>
            {
                Err(error.into())
            }
            _ => {
                forget(&tunnel.files);
                Ok(())
            }
        }
    }

    pub async fn states(
        &self,
        active: &ActiveId,
    ) -> Result<impl Stream<Item = TunnelState> + Send + Unpin + 'static> {
        let proxy = ActiveConnectionProxy::builder(&self.connection)
            .path(active.as_str().to_owned())?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        let changes = proxy.receive_state_changed().await?;
        let first = match proxy.current_state().await {
            Ok(code) => TunnelState::from_codes(code, 0),
            Err(_) => TunnelState::gone(),
        };
        let later = changes.filter_map(|signal| {
            ready(
                signal
                    .args()
                    .ok()
                    .map(|change| TunnelState::from_codes(change.state, change.reason)),
            )
        });

        Ok(stream::once(ready(first))
            .chain(later)
            .scan(false, |ended, state| {
                if *ended {
                    return ready(None);
                }
                *ended = state.state == ActiveState::Deactivated;
                ready(Some(state))
            })
            .boxed())
    }
}

fn names(error: &zbus::Error, error_names: &[&str]) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => error_names.contains(&name.as_str()),
        zbus::Error::FDO(fdo) => {
            let name = match fdo.as_ref() {
                zbus::fdo::Error::UnknownObject(_) => UNKNOWN_OBJECT,
                zbus::fdo::Error::UnknownMethod(_) => UNKNOWN_METHOD,
                _ => return false,
            };
            error_names.contains(&name)
        }
        _ => false,
    }
}

fn forget(files: &[PathBuf]) {
    for file in files {
        match std::fs::remove_file(file) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                tracing::warn!(file = %file.display(), %error, "failed to remove tunnel file");
            }
            _ => {}
        }
    }
}
fn path(path: &str) -> Result<ObjectPath<'static>> {
    Ok(ObjectPath::try_from(path.to_owned())?)
}
