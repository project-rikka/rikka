mod keys;
mod targets;
mod wire;

use std::sync::LazyLock;

use browser_translate::{
    Adapter, Call, Dialect, Error, Http, Limits, Result, Service, Translated, Translating,
};

use keys::{KeyCache, find_key};

const LABEL: &str = "Google";
const KEY_HEADER: &str = "x-goog-api-key";
const TRANSLATE: &str = "https://translate-pa.googleapis.com/v1/translateHtml";
const KEY_PAGE: &str = "https://translate.googleapis.com/_/translate_http/_/js/k=translate_http.tr.en_US.YusFYy3P_ro.O/am=AAg/d=1/exm=el_conf/ed=1/rs=AN8SPfq1Hb8iJRleQqQc8zhdzXmF9E56eQ/m=el_main";

static SERVICE: Service = Service {
    id: "google",
    label: LABEL,
    dialect: Dialect::new("i").keep("translate=\"no\""),
    targets: targets::TARGETS,
    aliases: &[
        ("zh-Hans", "zh-CN"),
        ("zh-Hant", "zh-TW"),
        ("fil", "tl"),
        ("nb", "no"),
        ("prs", "fa-AF"),
    ],
    limits: Limits::new(100, 5_000),
};

static KEYS: LazyLock<KeyCache> = LazyLock::new(KeyCache::default);

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Google;

impl Adapter for Google {
    fn service(&self) -> &'static Service {
        &SERVICE
    }

    fn translate<'a>(&'a self, http: &'a Http, call: Call<'a>) -> Translating<'a> {
        Box::pin(translate(http, call))
    }
}

async fn translate(http: &Http, call: Call<'_>) -> Result<Vec<Translated>> {
    let mut retried = false;
    loop {
        let key = KEYS
            .get(|| scrape(http))
            .await
            .ok_or(Error::KeyUnavailable(LABEL))?;
        let answer = http
            .post(TRANSLATE)
            .header("content-type", "application/json+protobuf")
            .header(KEY_HEADER, key.as_str())
            .body(wire::body(&call))
            .send()
            .await;
        match answer {
            Ok(body) => return wire::parse(&body).ok_or(Error::MalformedResponse),
            Err(Error::Refused) if !retried => {
                retried = true;
                KEYS.invalidate(&key).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn scrape(http: &Http) -> Option<String> {
    match http.page(KEY_PAGE).await {
        Ok(text) => find_key(&text, KEY_HEADER).map(str::to_owned),
        Err(error) => {
            tracing::warn!(%error, "failed to read Google access key page");
            None
        }
    }
}
