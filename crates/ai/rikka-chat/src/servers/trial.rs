use rikka_mcp::{Hub, Server, ServerStatus};

use crate::servers::offered::{Listed, Offered};

#[derive(Debug)]
pub(crate) enum Trial {
    Idle,
    Trying(Server),
    Reached(Vec<Listed>),
    Locked,
    Failed(String),
}

impl Trial {
    pub(crate) fn is_trying(&self) -> bool {
        matches!(self, Trial::Trying(_))
    }

    pub(crate) async fn run(hub: Hub, server: Server) -> Trial {
        Trial::from_standing(&hub.probe(server).await)
    }

    fn from_standing(standing: &ServerStatus) -> Trial {
        if standing.needs_sign_in() {
            return Trial::Locked;
        }
        match standing.error() {
            Some(trouble) => Trial::Failed(trouble.to_owned()),
            None => Trial::Reached(Offered::Tools.listed(standing)),
        }
    }
}
