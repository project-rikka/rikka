use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::api::time;
use crate::channel::Link;
use crate::error::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DataKinds {
    pub history: bool,
    pub cookies: bool,
    pub cache: bool,
    pub downloads: bool,
    pub form_data: bool,
    pub passwords: bool,
    pub site_storage: bool,
}

impl DataKinds {
    fn request(self) -> Value {
        let mut kinds = Map::new();
        let mut set = |name: &str, enabled: bool| {
            if enabled {
                kinds.insert(name.into(), Value::Bool(true));
            }
        };
        set("history", self.history);
        set("cookies", self.cookies);
        set("cache", self.cache);
        set("cacheStorage", self.cache);
        set("downloads", self.downloads);
        set("formData", self.form_data);
        set("passwords", self.passwords);
        for storage in [
            "localStorage",
            "indexedDB",
            "serviceWorkers",
            "fileSystems",
            "webSQL",
        ] {
            set(storage, self.site_storage);
        }
        Value::Object(kinds)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub session: bool,
    pub expiration_date: Option<f64>,
    #[serde(default)]
    pub same_site: String,
}

#[derive(Clone, Debug)]
pub struct Storage {
    link: Arc<Link>,
}

impl Storage {
    pub(crate) fn new(link: Arc<Link>) -> Storage {
        Storage { link }
    }

    pub fn clear_since(
        &self,
        since: SystemTime,
        kinds: DataKinds,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                "browsingData.remove",
                vec![json!({ "since": time::to_millis(since) }), kinds.request()],
            )
            .done()
    }

    pub fn clear_sites(
        &self,
        origins: &[String],
        kinds: DataKinds,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                "browsingData.remove",
                vec![json!({ "origins": origins }), kinds.request()],
            )
            .done()
    }

    pub fn cookies(&self, url: &str) -> impl Future<Output = Result<Vec<Cookie>>> + Send + use<> {
        self.link
            .call("cookies.getAll", vec![json!({ "url": url })])
            .decoded()
    }
}
