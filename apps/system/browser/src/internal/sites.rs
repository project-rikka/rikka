use std::sync::Arc;

use browser_pages::Origin;
use browser_store::{OriginDecision, ProfileId, SiteSettings, Store};
use rikka_app::prelude::*;
use rikka_web::{Partition, PermissionValue, Runtime};
use rikka_widgets::{column, row};

use crate::internal::status::Status;
use crate::shell::Shell;
use crate::site_info::{ask_name, decision_at, decision_choices, decision_index};
use browser_model::ProfileKey;
use browser_model::{Ask, Requester, Scope};

const LOAD: &str = "rikka-browser-sites";
const SCHEMES: [&str; 2] = ["https", "http"];
const CHOICE: f32 = 120.0;

#[derive(State)]
pub(crate) struct SitesPage {
    #[untracked]
    profile: Option<ProfileId>,
    sites: Tracked<Arc<Vec<SiteSettings>>>,
    status: Tracked<Status>,
}

impl SitesPage {
    pub(crate) fn new() -> SitesPage {
        SitesPage {
            profile: None,
            sites: Tracked::new(Arc::new(Vec::new())),
            status: Tracked::new(Status::Waiting),
        }
    }

    pub(crate) fn open(&mut self, store: Store, profile: Option<ProfileId>) {
        self.profile = profile;
        match profile {
            Some(profile) => load(store, profile),
            None => self.status.set(Status::Ready),
        }
    }

    fn loaded(&mut self, read: browser_store::Result<Vec<SiteSettings>>) {
        match read {
            Ok(sites) => {
                self.sites.set(Arc::new(sites));
                self.status.set(Status::Ready);
            }
            Err(error) => {
                tracing::warn!(%error, "failed to read site settings");
                self.status.set(Status::Failed);
            }
        }
    }
}

fn load(store: Store, profile: ProfileId) {
    spawn_blocking_keyed(
        LOAD,
        move || {
            store.flush()?;
            store.sites_with_settings(profile)
        },
        |page: &mut SitesPage, read: browser_store::Result<Vec<SiteSettings>>| page.loaded(read),
    );
}

impl std::fmt::Debug for SitesPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SitesPage")
            .field("sites", &self.sites.len())
            .finish_non_exhaustive()
    }
}

impl Shell {
    fn sites_changed(&mut self) {
        if let Some(profile) = self.persistent_profile() {
            load(self.browser().store(), profile);
        }
    }

    fn set_origin_permission(&mut self, requester: &Requester, ask: Ask, at: usize) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        self.remember_decision(Scope::Profile(profile), requester, ask, decision_at(at));
        self.sites_changed();
    }

    fn reset_site_zoom(&mut self, host: String) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        self.browser().edit().zooms.set(profile, &host, None);
        self.browser().store().clear_zoom(profile, &host);
        self.refit_zoom();
        self.sites_changed();
    }

    fn forbid_http_from_settings(&mut self, host: String) {
        if let Some(profile) = self.persistent_profile() {
            self.browser().forbid_http(profile, &host);
            self.sites_changed();
        }
    }

    fn reset_site(&mut self, site: SiteSettings) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        let host = site.host.clone();
        if site.https_only_exception {
            self.browser().forbid_http(profile, &host);
        }
        let forgotten = self
            .browser()
            .page_services()
            .grants_mut()
            .forget_site(Scope::Profile(profile), &host);
        match self.browser().engine(ProfileKey::Persistent(profile)) {
            Ok(runtime) => {
                for (requester, ask) in forgotten {
                    crate::requests::share(&runtime, &requester, ask, None);
                }
                reset_engine_permissions(&runtime, host.clone());
            }
            Err(error) => tracing::warn!(%error, "failed to reset site permissions"),
        }
        self.browser().edit().zooms.set(profile, &host, None);
        self.browser().store().clear_site(profile, &host);
        self.refit_zoom();
        self.sites_changed();
    }
}

fn reset_engine_permissions(runtime: &Runtime, host: String) {
    let storage = runtime.storage();
    let partition = Partition {
        context: None,
        private: runtime.is_ephemeral(),
    };
    spawn(
        async move {
            for scheme in SCHEMES {
                let uri = format!("{scheme}://{host}");
                for permission in storage.permissions_for(uri, &partition).await? {
                    storage.set_permission(&permission, PermissionValue::Unknown, false);
                }
            }
            Ok(())
        },
        |_: &mut Shell, reset: rikka_web::Result<()>| {
            if let Err(error) = reset {
                tracing::warn!(%error, "failed to reset site permissions");
            }
        },
    );
}

fn origin_label(decided: &OriginDecision, host: &str) -> Option<String> {
    if let Some(top) = &decided.top_site {
        return Some(tr!("browser.sites.embedded", site = top).to_string());
    }
    let origin = Origin::parse(&decided.origin)?;
    let plain = origin.scheme == "https" && origin.port.is_none() && origin.host == host;
    (!plain).then(|| decided.origin.clone())
}

fn site_card(site: &SiteSettings) -> Element {
    let host = site.host.clone();
    let permissions = site.permissions.iter().filter_map(|decided| {
        let ask = Ask::from_stored(decided.permission)?;
        let requester = Requester {
            origin: decided.origin.clone(),
            top_site: decided.top_site.clone(),
        };
        let named = origin_label(decided, &site.host);
        Some(
            field(
                ask_name(ask),
                row! {
                    named.map(|named| text(named).caption().muted().no_wrap().shrink(1.0).min_width(0.0));
                    combo(Some(decision_index(Some(decided.decision))))
                        .options(decision_choices())
                        .on_select(move |shell: &mut Shell, at: usize| {
                            shell.set_origin_permission(&requester, ask, at)
                        })
                        .width(CHOICE);
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .any(),
        )
    });
    let zoom = site.zoom.map(|zoom| {
        let host = host.clone();
        field(
            tr!(
                "browser.site.zoom",
                percent = &format!("{:.0}", zoom * 100.0)
            ),
            button(text(tr!("browser.site.reset")))
                .secondary()
                .small()
                .on_press(move |shell: &mut Shell| shell.reset_site_zoom(host.clone())),
        )
        .any()
    });
    let http = site.https_only_exception.then(|| {
        let host = host.clone();
        field(
            tr!("browser.sites.http"),
            switch(true).on_toggle(move |shell: &mut Shell, on: bool| {
                if !on {
                    shell.forbid_http_from_settings(host.clone());
                }
            }),
        )
        .any()
    });
    let reset = site.clone();
    section(
        site.host.clone(),
        column! {
            column_of(permissions).gap(Space::TIGHT).fill_width();
            zoom;
            http;
            row! {
                button(text(tr!("browser.sites.reset")))
                    .secondary()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.reset_site(reset.clone()));
            };
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
    .any()
}

pub(crate) fn view(shell: &Shell) -> Element {
    let entry = &shell.pages().sites;
    if entry.profile.is_none() {
        return status_page(MaterialSymbol::TUNE, tr!("browser.sites.private")).any();
    }
    match (*entry.status, entry.sites.is_empty()) {
        (Status::Failed, _) => {
            status_page(MaterialSymbol::WARNING, tr!("browser.sites.unreadable")).any()
        }
        (Status::Waiting, true) => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        (_, true) => status_page(MaterialSymbol::TUNE, tr!("browser.sites.empty")).any(),
        (_, false) => page(
            entry
                .sites
                .iter()
                .map(|site| site_card(site).keyed(site.host.clone())),
        )
        .any(),
    }
}
