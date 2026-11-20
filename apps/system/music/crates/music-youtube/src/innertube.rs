use rikka_sync::Mutex;

use music_core::{Error, Result};
use music_stream::{Scan, answer_text, api_client};
use rikka_net::Client as Http;
use serde_json::{Value, json};

use crate::clients::Client;
use crate::jar::Jar;

const LANGUAGE: &str = "en";
const VISITOR_PAGE: &str = "https://www.youtube.com/sw.js_data";

#[derive(Debug)]
pub(crate) struct Innertube {
    http: Http,
    jar: Jar,
    region: String,
    visitor: Mutex<Option<String>>,
}

pub(crate) enum As<'a> {
    Account,
    Guest(Option<&'a str>),
}

impl Innertube {
    pub(crate) fn new(jar: Jar, region: String) -> Innertube {
        Innertube {
            http: api_client(Client::Music.agent()),
            jar,
            region,
            visitor: Mutex::new(None),
        }
    }

    pub(crate) async fn call(
        &self,
        endpoint: &str,
        client: Client,
        payload: Value,
    ) -> Result<Value> {
        self.post(endpoint, client, payload, As::Account).await
    }

    pub(crate) async fn post(
        &self,
        endpoint: &str,
        client: Client,
        mut payload: Value,
        who: As<'_>,
    ) -> Result<Value> {
        let visitor = match who {
            As::Guest(visitor) => visitor,
            As::Account => None,
        };
        payload["context"] = client.context(visitor, LANGUAGE, &self.region);
        if client == Client::Music {
            payload["isAudioOnly"] = json!(true);
        }
        let origin = client.origin();
        let mut request = self
            .http
            .post(format!(
                "{origin}/youtubei/v1/{endpoint}?prettyPrint=false&alt=json"
            ))
            .header("Accept", "*/*")
            .header("Origin", origin)
            .header("User-Agent", client.agent())
            .header("X-Youtube-Client-Name", client.number().to_string())
            .header("X-Youtube-Client-Version", client.version())
            .json(&payload);
        let jar = match who {
            As::Account => {
                let jar = &self.jar;
                let authorization = jar
                    .authorization(origin)
                    .ok_or_else(|| Error::Refused("the YouTube cookies are missing".into()))?;
                request = request
                    .header("Cookie", jar.header())
                    .header("Authorization", authorization)
                    .header("X-Origin", origin)
                    .header("X-Goog-AuthUser", "0");
                Some(jar)
            }
            As::Guest(Some(visitor)) => {
                request = request.header("X-Goog-Visitor-Id", visitor);
                None
            }
            As::Guest(None) => None,
        };
        let response = request.send().await.map_err(Error::unreachable)?;
        let status = response.status().as_u16();
        if let Some(jar) = jar {
            let set: Vec<String> = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .filter_map(|value| value.to_str().ok().map(str::to_owned))
                .collect();
            jar.absorb(set).await;
        }
        let bytes = response.bytes().await.map_err(Error::unreachable)?;
        let answer: Value = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Unreadable(format!("{endpoint} answered {status} with no JSON")))?;
        if let Some(error) = answer.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("no reason given")
                .to_owned();
            return Err(Error::from_status(status, format!("{endpoint}: {message}")));
        }
        if !(200..300).contains(&status) {
            return Err(Error::from_status(
                status,
                format!("{endpoint} answered {status}"),
            ));
        }
        Ok(answer)
    }

    pub(crate) async fn visitor(&self) -> Option<String> {
        if let Some(known) = self.visitor_id().clone() {
            return Some(known);
        }
        match self.issued_visitor().await {
            Ok(issued) => {
                *self.visitor_id() = Some(issued.clone());
                Some(issued)
            }
            Err(error) => {
                tracing::warn!("no YouTube visitor id: {error}");
                None
            }
        }
    }

    pub(crate) fn adopt_visitor(&self, visitor: String) {
        *self.visitor_id() = Some(visitor);
    }

    async fn issued_visitor(&self) -> Result<String> {
        let request = self
            .http
            .get(VISITOR_PAGE)
            .header("User-Agent", Client::VisionOs.agent());
        let page = answer_text(request).await?;
        visitor_in(&page).ok_or_else(|| Error::Unreadable("no visitor id".into()))
    }

    fn visitor_id(&self) -> rikka_sync::MutexGuard<'_, Option<String>> {
        self.visitor.lock()
    }
}

const VISITOR_PREFIX: &str = "Cg";
const VISITOR_LEAST: usize = 20;

fn visitor_in(page: &str) -> Option<String> {
    Scan::new(page)
        .matches(&format!("\"{VISITOR_PREFIX}"), |mut scan| {
            let rest = scan.take(VISITOR_LEAST..=usize::MAX, |c| {
                c.is_ascii_alphanumeric() || "%_+=-".contains(c)
            })?;
            scan.expect("\"")?;
            Some(format!("{VISITOR_PREFIX}{rest}"))
        })
        .next()
}
