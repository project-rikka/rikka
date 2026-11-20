use browser_store::SiteSettings;
use rikka_web::{HttpsOnly, Partition, PermissionValue, Runtime};

use crate::browser::Browser;
use browser_model::{ProfileId, ProfileKey};

const INSECURE: &str = "https-only-load-insecure";
const SCHEMES: [&str; 2] = ["http", "https"];

pub(crate) fn mode(on: bool) -> HttpsOnly {
    match on {
        true => HttpsOnly::Everywhere,
        false => HttpsOnly::Off,
    }
}

fn excepted(runtime: &Runtime, host: &str, value: PermissionValue) {
    let storage = runtime.storage();
    let partition = Partition {
        context: None,
        private: runtime.is_ephemeral(),
    };
    for scheme in SCHEMES {
        storage.set_permission_for(format!("{scheme}://{host}"), INSECURE, value, &partition);
    }
}

pub(crate) fn allow_http_on(runtime: &Runtime, sites: &[SiteSettings]) {
    for site in sites.iter().filter(|site| site.https_only_exception) {
        excepted(runtime, &site.host, PermissionValue::Allow);
    }
}

impl Browser {
    pub(crate) fn set_https_only(&self, on: bool) {
        self.preferences().set_https_only(on);
        for (key, engine) in self.live_engines() {
            engine.set_preferences(&self.web_preferences(key));
        }
    }

    pub(crate) fn allow_http(&self, key: ProfileKey, host: &str) -> rikka_web::Result<()> {
        let runtime = self.engine(key)?;
        excepted(&runtime, host, PermissionValue::Allow);
        if let ProfileKey::Persistent(profile) = key {
            self.store().set_https_only_exception(profile, host, true);
        }
        Ok(())
    }

    pub(crate) fn forbid_http(&self, profile: ProfileId, host: &str) {
        self.store().set_https_only_exception(profile, host, false);
        let key = ProfileKey::Persistent(profile);
        for (_, engine) in self
            .live_engines()
            .into_iter()
            .filter(|(candidate, _)| *candidate == key)
        {
            excepted(&engine, host, PermissionValue::Unknown);
        }
    }
}
