use std::time::Duration;

use rikka_net::{Client, RetryPolicy};

use crate::error::Result;
use crate::pins;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub(crate) struct Transports {
    pub(crate) plain: Client,
    pub(crate) api: Client,
    pub(crate) alternative: Client,
}

impl Transports {
    pub(crate) fn new(
        user_agent: &str,
        timeout: Option<Duration>,
        pinned: bool,
        alternative_routing: bool,
    ) -> Result<Transports> {
        let plain = client(user_agent, timeout, Trust::Authorities)?;
        if !pinned {
            return Ok(Transports {
                api: plain.clone(),
                alternative: plain.clone(),
                plain,
            });
        }

        let api = client(user_agent, timeout, Trust::PinnedChain)?;
        let alternative = match alternative_routing {
            true => client(user_agent, timeout, Trust::PinnedLeaf)?,
            false => api.clone(),
        };
        Ok(Transports {
            plain,
            api,
            alternative,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum Trust {
    Authorities,
    PinnedChain,
    PinnedLeaf,
}

fn client(user_agent: &str, timeout: Option<Duration>, trust: Trust) -> Result<Client> {
    let mut client = Client::builder()
        .user_agent(user_agent)
        .connect_timeout(CONNECT_TIMEOUT)
        .retry(RetryPolicy::never());
    if let Some(timeout) = timeout {
        client = client.timeout(timeout);
    }
    client = match trust {
        Trust::Authorities => client,
        Trust::PinnedChain => client.pin_public_keys(pins::api()),
        Trust::PinnedLeaf => client.trust_leaf_public_keys(pins::alternative_api()),
    };
    Ok(client.build()?)
}
