use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, header};
use url::Url;

use crate::api::{ACCEPT, Api, Config};
use crate::backoff::{Backoff, DEFAULT_RETRIES};
use crate::doh::Resolver;
use crate::environment::Environment;
use crate::error::{Error, Result};
use crate::routing::{self, Routing};
use crate::session::Session;
use crate::store::SessionStore;
use crate::transport::Transports;
use crate::version::AppVersion;

const DEFAULT_LOCALE: &str = "en";
const SECURE_SCHEME: &str = "https";
const APP_VERSION: HeaderName = HeaderName::from_static("x-pm-appversion");
const LOCALE: HeaderName = HeaderName::from_static("x-pm-locale");

#[must_use]
#[derive(Debug)]
pub struct ApiBuilder {
    environment: Environment,
    app_version: Option<AppVersion>,
    user_agent: Option<String>,
    locale: String,
    headers: HeaderMap,
    timeout: Option<Duration>,
    retries: u32,
    certificate_pinning: bool,
    alternative_routing: bool,
    doh_providers: Option<Vec<String>>,
    store: Option<Arc<dyn SessionStore>>,
    session: Option<Session>,
    error: Option<Error>,
}

impl ApiBuilder {
    pub(crate) fn new() -> ApiBuilder {
        ApiBuilder {
            environment: Environment::production(),
            app_version: None,
            user_agent: None,
            locale: DEFAULT_LOCALE.to_owned(),
            headers: HeaderMap::new(),
            timeout: None,
            retries: DEFAULT_RETRIES,
            certificate_pinning: true,
            alternative_routing: false,
            doh_providers: None,
            store: None,
            session: None,
            error: None,
        }
    }

    pub fn environment(mut self, environment: Environment) -> Self {
        self.environment = environment;
        self
    }

    pub fn app_version(mut self, version: impl AsRef<str>) -> Self {
        match AppVersion::parse(version.as_ref()) {
            Ok(version) => self.app_version = Some(version),
            Err(error) => return self.fail(error),
        }
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        let (Ok(name), Ok(value)) = (name.try_into(), value.try_into()) else {
            return self.fail(Error::InvalidHeader);
        };

        self.headers.insert(name, value);
        self
    }

    pub fn session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    pub fn session_store(mut self, store: impl SessionStore) -> Self {
        self.store = Some(Arc::new(store));
        self
    }

    pub fn certificate_pinning(mut self, enabled: bool) -> Self {
        self.certificate_pinning = enabled;
        self
    }

    pub fn alternative_routing(mut self, enabled: bool) -> Self {
        self.alternative_routing = enabled;
        self
    }

    pub fn doh_providers(mut self, providers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut selected = Vec::new();
        for provider in providers {
            let provider = provider.into();
            let usable = Url::parse(&provider)
                .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host());
            if !usable {
                return self.fail(Error::InvalidUrl(provider));
            }
            selected.push(provider);
        }
        self.doh_providers = Some(selected);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    pub fn build(self) -> Result<Api> {
        let ApiBuilder {
            environment,
            app_version,
            user_agent,
            locale,
            headers: custom_headers,
            timeout,
            retries,
            certificate_pinning,
            alternative_routing,
            doh_providers,
            store,
            session,
            error,
        } = self;

        if let Some(error) = error {
            return Err(error);
        }

        let app_version = app_version.ok_or(Error::MissingAppVersion)?;
        let user_agent = user_agent.unwrap_or_else(|| default_user_agent(&app_version));
        let headers = default_headers(&app_version, &locale, &user_agent, custom_headers)?;

        let pinned = certificate_pinning && environment.base_url().scheme() == SECURE_SCHEME;
        let transports = Transports::new(&user_agent, timeout, pinned, alternative_routing)?;

        let session = match (session, &store) {
            (Some(session), _) => Some(session),
            (None, Some(store)) => store.load()?,
            (None, None) => None,
        };

        let config = Config {
            routing: alternative_routing.then(|| {
                Routing::new(
                    Resolver::new(transports.plain.clone(), doh_providers),
                    transports.alternative,
                    routing::VALIDITY,
                )
            }),
            http: transports.plain,
            api_http: transports.api,
            environment,
            app_version,
            headers,
            timeout,
            backoff: Backoff::new(retries),
        };
        Ok(Api::new(config, store, session))
    }

    fn fail(mut self, error: Error) -> Self {
        if self.error.is_none() {
            self.error = Some(error);
        }
        self
    }
}

impl Default for ApiBuilder {
    fn default() -> ApiBuilder {
        ApiBuilder::new()
    }
}

fn default_user_agent(version: &AppVersion) -> String {
    format!(
        "{}/{} ({}; {})",
        version.client(),
        version.version(),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn default_headers(
    app_version: &AppVersion,
    locale: &str,
    user_agent: &str,
    custom: HeaderMap,
) -> Result<HeaderMap> {
    let value = |text: &str| HeaderValue::from_str(text).map_err(|_| Error::InvalidHeader);
    let mut headers = HeaderMap::new();
    headers.insert(APP_VERSION, value(app_version.as_str())?);
    headers.insert(LOCALE, value(locale)?);
    headers.insert(header::USER_AGENT, value(user_agent)?);
    headers.insert(header::ACCEPT, HeaderValue::from_static(ACCEPT));
    headers.extend(custom);
    Ok(headers)
}
