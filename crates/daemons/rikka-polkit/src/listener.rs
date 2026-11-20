use std::collections::HashMap;
use std::sync::Arc;

use crate::identity::{self, RawIdentity};
use crate::pending::{Opening, Outcome, Pending};

pub(crate) const PATH: &str = "/org/rikka/PolicyKit1/AuthenticationAgent";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
pub(crate) enum Refusal {
    Cancelled(String),
    Failed(String),
}

pub(crate) struct Listener {
    pending: Arc<Pending>,
}

impl Listener {
    pub(crate) fn new(pending: Arc<Pending>) -> Listener {
        Listener { pending }
    }
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Listener {
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        details: HashMap<String, String>,
        cookie: String,
        identities: Vec<RawIdentity>,
    ) -> Result<(), Refusal> {
        let users = identity::users(&identities);
        if users.is_empty() {
            return Err(Refusal::Failed(
                "nobody who could authorize that can be authenticated here".to_owned(),
            ));
        }
        tracing::info!("authentication requested for `{action_id}`");

        let reply = self.pending.open(Opening {
            action: action_id,
            message,
            details: visible_details(details),
            users,
            cookie,
        });
        match reply.await {
            Ok(Outcome::Authorized | Outcome::Denied) => Ok(()),
            Ok(Outcome::Cancelled) | Err(_) => {
                Err(Refusal::Cancelled("nobody authenticated".to_owned()))
            }
        }
    }

    async fn cancel_authentication(&self, cookie: String) {
        self.pending.withdraw(&cookie);
    }
}

fn visible_details(details: HashMap<String, String>) -> Vec<(String, String)> {
    let mut visible_details: Vec<(String, String)> = details
        .into_iter()
        .filter(|(key, _)| !key.starts_with("polkit."))
        .collect();
    visible_details.sort();
    visible_details
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").finish_non_exhaustive()
    }
}
