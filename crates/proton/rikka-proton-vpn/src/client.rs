use std::time::Duration;

use jiff::Timestamp;
use rikka_proton_account::Account;
use rikka_proton_api::{Api, percent};

use crate::account::{ClientConfig, VpnAccount, VpnSession};
use crate::certificate::{Certificate, CertificateRequest};
use crate::config::Protocol;
use crate::error::Result;
use crate::servers::{
    BinaryStatus, CityNames, ConnectingDomain, LogicalServer, Logicals, ServerCounts,
    StreamingServices, UserLocation,
};
use crate::signin::{self, SignInCode, SignedIn};
use crate::time;

const LOGICALS: &str = "vpn/v2/logicals";
const STATUS: &str = "vpn/v2/status";
const LOOKUP: &str = "vpn/v1/logicals/lookup";
const SERVERS: &str = "vpn/v1/servers";
const COUNTS: &str = "vpn/v1/servers-count";
const STREAMING: &str = "vpn/v1/streamingservices";
const CITIES: &str = "vpn/v1/cities/names";
const LOCATION: &str = "vpn/v1/location";
const ACCOUNT: &str = "vpn/v2";
const SESSIONS: &str = "vpn/v1/sessions";
const CLIENT_CONFIG: &str = "vpn/v2/clientconfig";
const CERTIFICATE: &str = "vpn/v1/certificate";

const NETZONE_HEADER: &str = "x-pm-netzone";
const LOCALE_HEADER: &str = "x-pm-locale";
const TRUNCATION_HEADER: &str = "x-pm-response-truncation-permitted";
const MODIFIED_SINCE_HEADER: &str = "if-modified-since";
const LAST_MODIFIED_HEADER: &str = "last-modified";

const LOGICALS_TIMEOUT: Duration = Duration::from_secs(20);
const NOT_MODIFIED: u16 = 304;

#[derive(Debug, Clone)]
pub struct VpnClient {
    api: Api,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct LogicalsRequest {
    api: Api,
    protocols: Vec<Protocol>,
    include: Vec<String>,
    with_state: bool,
    translations: Option<String>,
    netzone: Option<String>,
    modified_since: Option<Timestamp>,
    truncatable: bool,
}

#[derive(Debug, Clone)]
pub enum ServerList {
    Changed {
        logicals: Logicals,
        last_modified: Option<Timestamp>,
    },
    Unchanged,
}

impl VpnClient {
    pub fn new(api: Api) -> VpnClient {
        VpnClient { api }
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        &self.api
    }

    pub fn logicals(&self) -> LogicalsRequest {
        LogicalsRequest {
            api: self.api.clone(),
            protocols: Vec::new(),
            include: Vec::new(),
            with_state: true,
            translations: None,
            netzone: None,
            modified_since: None,
            truncatable: false,
        }
    }

    pub async fn servers(&self) -> Result<Vec<LogicalServer>> {
        Ok(match self.logicals().send().await? {
            ServerList::Changed { logicals, .. } => logicals.servers,
            ServerList::Unchanged => Vec::new(),
        })
    }

    pub async fn status(&self, status_id: &str) -> Result<BinaryStatus> {
        let bytes = self
            .api
            .get(format!("{STATUS}/{}/binary", percent::encode(status_id)))
            .send_bytes()
            .await?;

        BinaryStatus::parse(bytes)
    }

    pub async fn lookup(&self, name: &str) -> Result<Option<LogicalServer>> {
        let response: Option<LogicalServer> = self
            .api
            .get(format!("{LOOKUP}/{}", percent::encode(name)))
            .send_field_or_default("LogicalServer")
            .await?;

        Ok(response)
    }

    pub async fn server(&self, id: &str) -> Result<ConnectingDomain> {
        let response: ConnectingDomain = self
            .api
            .get(format!("{SERVERS}/{}", percent::encode(id)))
            .send_field("Server")
            .await?;

        Ok(response)
    }

    pub async fn server_counts(&self) -> Result<ServerCounts> {
        Ok(self.api.get(COUNTS).send().await?)
    }

    pub async fn streaming_services(&self) -> Result<StreamingServices> {
        Ok(self.api.get(STREAMING).send().await?)
    }

    pub async fn city_names(&self, language: &str) -> Result<CityNames> {
        Ok(self
            .api
            .get(CITIES)
            .header(LOCALE_HEADER, language)
            .send()
            .await?)
    }

    pub async fn location(&self) -> Result<UserLocation> {
        Ok(self.api.get(LOCATION).send().await?)
    }

    pub async fn vpn_info(&self) -> Result<VpnAccount> {
        Ok(self.api.get(ACCOUNT).send().await?)
    }

    pub async fn sessions(&self) -> Result<Vec<VpnSession>> {
        Ok(self
            .api
            .get(SESSIONS)
            .send_field_or_default("Sessions")
            .await?)
    }

    pub async fn client_config(&self) -> Result<ClientConfig> {
        Ok(self.api.get(CLIENT_CONFIG).send().await?)
    }

    pub async fn certificate(&self, request: &CertificateRequest) -> Result<Certificate> {
        Ok(self
            .api
            .post(CERTIFICATE)
            .json(&request.body())
            .send()
            .await?)
    }

    pub async fn sign_in_code(&self) -> Result<SignInCode> {
        signin::begin(&self.account()).await
    }

    pub async fn poll_sign_in(&self, code: &SignInCode) -> Result<Option<SignedIn>> {
        signin::poll(&self.account(), code).await
    }

    pub async fn await_sign_in(&self, code: &SignInCode) -> Result<SignedIn> {
        signin::wait(&self.account(), code).await
    }

    pub async fn web_sign_in_url(&self, url: &str) -> Result<String> {
        signin::web_url(&self.account(), url).await
    }

    fn account(&self) -> Account {
        Account::new(self.api.clone())
    }
}

impl LogicalsRequest {
    pub fn protocols(mut self, protocols: impl IntoIterator<Item = Protocol>) -> LogicalsRequest {
        self.protocols = protocols.into_iter().collect();
        self
    }

    pub fn include(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> LogicalsRequest {
        self.include = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_state(mut self, enabled: bool) -> LogicalsRequest {
        self.with_state = enabled;
        self
    }

    pub fn with_translations(mut self, language: impl Into<String>) -> LogicalsRequest {
        self.translations = Some(language.into());
        self
    }

    pub fn netzone(mut self, netzone: impl Into<String>) -> LogicalsRequest {
        self.netzone = Some(netzone.into());
        self
    }

    pub fn modified_since(mut self, at: Timestamp) -> LogicalsRequest {
        self.modified_since = Some(at);
        self
    }

    pub fn allow_truncation(mut self, allowed: bool) -> LogicalsRequest {
        self.truncatable = allowed;
        self
    }

    pub async fn send(self) -> Result<ServerList> {
        let mut request = self.api.get(LOGICALS).timeout(LOGICALS_TIMEOUT);

        if !self.protocols.is_empty() {
            let names: Vec<&str> = self
                .protocols
                .iter()
                .map(|protocol| protocol.api_name())
                .collect();
            request = request.query(&[("WithEntriesForProtocols", names.join(","))]);
        }

        request = request.query(&[("WithState", self.with_state)]);

        if let Some(language) = &self.translations {
            request = request.query(&[("WithTranslations", language)]);
        }

        for id in &self.include {
            request = request.query(&[("IncludeID[]", id)]);
        }

        if let Some(netzone) = &self.netzone {
            request = request.header(NETZONE_HEADER, netzone.as_str());
        }

        if let Some(at) = self.modified_since {
            request = request.header(MODIFIED_SINCE_HEADER, time::http_date(at));
        }

        if self.truncatable {
            request = request.header(TRUNCATION_HEADER, "true");
        }

        match request.receive::<Logicals>().await {
            Ok(received) => {
                let last_modified = received
                    .header(LAST_MODIFIED_HEADER)
                    .and_then(time::parse_http_date);
                Ok(ServerList::Changed {
                    logicals: received.into_value(),
                    last_modified,
                })
            }
            Err(error) if is_unchanged(&error) => Ok(ServerList::Unchanged),
            Err(error) => Err(error.into()),
        }
    }
}

impl From<Api> for VpnClient {
    fn from(api: Api) -> VpnClient {
        VpnClient::new(api)
    }
}

fn is_unchanged(error: &rikka_proton_api::Error) -> bool {
    error
        .status()
        .is_some_and(|status| status.as_u16() == NOT_MODIFIED)
}
