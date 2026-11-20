use rikka_net::HeaderMap;
use serde_json::Value;

use crate::allowance::window::Window;
use crate::error::Result;
use crate::request::Request;
use crate::transport::Transport;
use crate::wire::Reader;

#[derive(Clone, Debug)]
pub struct Meter {
    transport: Transport,
    body: Option<Value>,
    read: Reader,
}

impl Meter {
    pub(crate) fn new(transport: Transport, read: Reader) -> Meter {
        Meter {
            transport,
            body: None,
            read,
        }
    }

    pub(crate) fn posting(transport: Transport, body: Value, read: Reader) -> Meter {
        Meter {
            transport,
            body: Some(body),
            read,
        }
    }

    pub async fn windows(&self) -> Result<Vec<Window>> {
        let published = match &self.body {
            None => self.transport.get().await?,
            Some(body) => {
                self.transport
                    .post(body, &Request::default(), &HeaderMap::new())
                    .await?
            }
        };
        Ok((self.read)(&published))
    }
}
