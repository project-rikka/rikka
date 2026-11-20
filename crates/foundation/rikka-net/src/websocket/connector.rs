use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderValue, header};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Error as TungsteniteError;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{Connector as TlsConnector, MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::cancel::{self, CancelToken};
use crate::error::{Error, Result};
use crate::status_error::{BODY_LIMIT, StatusError};

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub(crate) struct Connector {
    pub(crate) url: Url,
    pub(crate) headers: HeaderMap,
    pub(crate) protocols: Vec<String>,
    pub(crate) config: WebSocketConfig,
    pub(crate) tls: Arc<rustls::ClientConfig>,
    pub(crate) connect_timeout: Option<Duration>,
}

impl Connector {
    pub(crate) async fn connect(
        &self,
        cancel: Option<&CancelToken>,
    ) -> Result<(Socket, Option<String>)> {
        let mut request = self.url.as_str().into_client_request()?;
        let headers = request.headers_mut();

        for (name, value) in &self.headers {
            headers.insert(name.clone(), value.clone());
        }
        if !self.protocols.is_empty() {
            let value = HeaderValue::from_str(&self.protocols.join(", "))
                .map_err(|_| Error::InvalidHeader("invalid subprotocol names".to_owned()))?;
            headers.insert(header::SEC_WEBSOCKET_PROTOCOL, value);
        }

        let handshake = tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(self.config),
            false,
            Some(TlsConnector::Rustls(Arc::clone(&self.tls))),
        );

        let handshake = match self.connect_timeout {
            Some(limit) => cancel::until_cancelled(tokio::time::timeout(limit, handshake), cancel)
                .await?
                .map_err(|_| Error::Timeout)?,
            None => cancel::until_cancelled(handshake, cancel).await?,
        };

        let (socket, response) = handshake.map_err(|error| self.rejected(error))?;

        let protocol = response
            .headers()
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        Ok((socket, protocol))
    }

    fn rejected(&self, error: TungsteniteError) -> Error {
        let TungsteniteError::Http(response) = error else {
            return Error::from(error);
        };

        let body = response
            .body()
            .as_deref()
            .map_or_else(String::new, |bytes| {
                String::from_utf8_lossy(&bytes[..bytes.len().min(BODY_LIMIT)]).into_owned()
            });

        StatusError::new(
            response.status(),
            self.url.clone(),
            response.headers(),
            body,
        )
        .into()
    }
}
