mod view;

pub(crate) use view::{
    ask_name, badge, capturing, decision_at, decision_choices, decision_index, peek_badge,
    security_glyph, view,
};

use browser_pages::Origin;
use browser_store::Decision;
use rikka_app::prelude::*;
use rikka_web::ClearFlags;

use crate::requests::decided_here;
use crate::shell::Shell;
use browser_model::TabId;
use browser_model::{Ask, Requester};

pub(crate) const BADGE: &str = "rikka-browser-site-badge";
pub(crate) const PEEK_BADGE: &str = "rikka-browser-peek-badge";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Clearing {
    #[default]
    Idle,
    Busy,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Capture {
    Screen,
    Devices,
}

#[derive(State)]
pub(crate) struct SiteInfo {
    certificate: Tracked<bool>,
    clearing: Tracked<Clearing>,
}

impl SiteInfo {
    pub(crate) fn new() -> SiteInfo {
        SiteInfo {
            certificate: Tracked::new(false),
            clearing: Tracked::new(Clearing::Idle),
        }
    }

    pub(crate) fn certificate_open(&self) -> bool {
        *self.certificate
    }

    pub(crate) fn clearing(&self) -> Clearing {
        *self.clearing
    }

    pub(crate) fn toggle_certificate(&mut self) {
        let open = !*self.certificate;
        self.certificate.set(open);
    }
}

impl std::fmt::Debug for SiteInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SiteInfo")
            .field("clearing", &*self.clearing)
            .finish_non_exhaustive()
    }
}

impl Shell {
    pub(crate) fn reset_site_info(&mut self) {
        self.overlays.site.certificate.set(false);
        self.overlays.site.clearing.set(Clearing::Idle);
    }

    pub(crate) fn clear_site_data(&mut self, tab: TabId) {
        let Some((key, url)) = self
            .model()
            .tab(tab)
            .map(|tab_entry| (tab_entry.profile, (*tab_entry.meta.url).clone()))
        else {
            return;
        };
        let Some(site) = browser_pages::registrable_site(&url) else {
            return;
        };
        let runtime = match self.browser().engine(key) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to clear site data");
                return;
            }
        };
        self.overlays.site.clearing.set(Clearing::Busy);
        spawn(
            async move {
                runtime
                    .storage()
                    .clear_base_domain(site, ClearFlags::SITE_DATA)
                    .await
            },
            move |shell: &mut Shell, cleared: rikka_web::Result<()>| {
                if let Err(error) = cleared {
                    tracing::warn!(%error, "failed to clear site data");
                }
                shell.overlays.site.clearing.set(Clearing::Done);
            },
        );
    }

    pub(crate) fn stop_capture(&mut self, tab: TabId, capture: Capture) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        match capture {
            Capture::Screen => view.stop_screen_sharing(),
            Capture::Devices => view.stop_capture(),
        }
    }

    pub(crate) fn set_site_permission(&mut self, tab: TabId, ask: Ask, decision: Option<Decision>) {
        let Some(origin) = self
            .model()
            .tab(tab)
            .and_then(|tab_entry| Origin::parse(&tab_entry.meta.url))
        else {
            return;
        };
        let Some(scope) = self.scope_of(tab) else {
            return;
        };
        self.remember_decision(scope, &Requester::origin(origin.to_string()), ask, decision);
    }

    pub(crate) fn site_permission(&self, tab: TabId, ask: Ask) -> Option<Decision> {
        let origin = self
            .model()
            .tab(tab)
            .and_then(|tab_entry| Origin::parse(&tab_entry.meta.url))?;
        decided_here(self, tab, &origin.to_string(), ask)
    }
}
