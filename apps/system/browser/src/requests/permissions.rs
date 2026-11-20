use browser_store::Decision;
use rikka_app::prelude::*;
use rikka_web::{Autoplay, MediaRequest, PermissionKind, PermissionRequest};
use rikka_widgets::{column, row};

use crate::requests::{Asking, PermissionPrompt, TabRequests};
use crate::shell::Shell;
use crate::widgets::glyph_button;
use browser_model::{Ask, Plan, Requester, Scope, content_asks, media_asks, plan};
use browser_model::{ProfileKey, TabId};

const WIDTH: f32 = 340.0;

impl Shell {
    pub(crate) fn permission_asked(&mut self, tab: TabId, request: PermissionRequest) {
        let autoplay = match request.kind() {
            Some(PermissionKind::AutoplayAudible) => Some(true),
            Some(PermissionKind::AutoplayInaudible) => Some(false),
            _ => None,
        };
        if let Some(audible) = autoplay {
            self.autoplay_asked(tab, request, audible);
            return;
        }
        let Some(requester) = Requester::from_content(&request.permission) else {
            request.deny();
            return;
        };
        let asks = content_asks(&request.permission);
        self.decide_asking(tab, Asking::Content(request), requester, asks);
    }

    pub(crate) fn media_asked(&mut self, tab: TabId, request: MediaRequest) {
        let Some(requester) = Requester::from_page(&request.uri) else {
            request.deny();
            return;
        };
        let asks = media_asks(&request);
        self.decide_asking(tab, Asking::Media(request), requester, asks);
    }

    fn autoplay_asked(&mut self, tab: TabId, request: PermissionRequest, audible: bool) {
        let Some(key) = self.model().tab(tab).map(|tab_entry| tab_entry.profile) else {
            request.deny();
            return;
        };
        let allowed = match self.browser().engine_settings(key).autoplay {
            Autoplay::Allowed => true,
            Autoplay::BlockAudible => !audible,
            Autoplay::BlockAll => false,
        };
        match allowed {
            true => request.allow(),
            false => request.deny(),
        }
    }

    fn decide_asking(
        &mut self,
        tab: TabId,
        request: Asking,
        requester: Requester,
        asks: Option<Vec<Ask>>,
    ) {
        let (Some(scope), Some(key)) = (
            self.scope_of(tab),
            self.model().tab(tab).map(|tab_entry| tab_entry.profile),
        ) else {
            request.deny();
            return;
        };
        let defaults = self.browser().engine_settings(key).permissions;
        let decided = {
            let grants = self.browser().page_services().grants();
            plan(asks, |ask| {
                grants
                    .decision(scope, &requester, ask)
                    .or_else(|| ask.preset_decision(&defaults))
            })
        };
        match decided {
            Plan::Allow => request.allow(),
            Plan::Deny => request.deny(),
            Plan::Prompt(asks) => {
                if let Asking::Content(content) = &request {
                    content.notify_shown();
                }
                let site = requester.host();
                self.browser()
                    .page_services()
                    .requests_mut()
                    .edit(tab)
                    .prompts
                    .push_back(PermissionPrompt {
                        request,
                        asks,
                        requester,
                        site,
                    });
            }
        }
    }

    pub(crate) fn answer_prompt(&mut self, tab: TabId, decision: Option<Decision>) {
        let Some(PermissionPrompt {
            request,
            asks,
            requester,
            ..
        }) = self
            .browser()
            .page_services()
            .requests_mut()
            .edit_existing(tab)
            .and_then(|requests| requests.prompts.pop_front())
        else {
            return;
        };
        match decision {
            Some(Decision::Allow) => request.allow(),
            Some(Decision::Deny) | None => request.deny(),
        }
        if let (Some(decision), Some(scope)) = (decision, self.scope_of(tab)) {
            for ask in asks {
                self.remember_decision(scope, &requester, ask, Some(decision));
            }
        }
    }

    pub(crate) fn remember_decision(
        &mut self,
        scope: Scope,
        requester: &Requester,
        ask: Ask,
        decision: Option<Decision>,
    ) {
        let changed = self
            .browser()
            .page_services()
            .grants_mut()
            .remember(scope, requester, ask, decision);
        if !changed {
            return;
        }
        let key = match scope {
            Scope::Profile(profile) => ProfileKey::Persistent(profile),
            Scope::Window(_) => ProfileKey::Private,
        };
        if let Some(runtime) = self.browser().running_engine(key) {
            crate::requests::share(&runtime, requester, ask, decision);
        }
        let Scope::Profile(profile) = scope else {
            return;
        };
        if let Some(permission) = ask.stored() {
            let store = self.browser().store();
            let origin = requester.origin.as_str();
            match (requester.top_site.as_deref(), decision) {
                (Some(top), Some(decision)) => {
                    store.set_storage_access(profile, origin, top, decision);
                }
                (Some(top), None) => store.clear_storage_access(profile, origin, top),
                (None, Some(decision)) => {
                    store.set_permission(profile, origin, permission, decision);
                }
                (None, None) => store.clear_permission(profile, origin, permission),
            }
        }
    }
}

pub(crate) fn decided_here(shell: &Shell, tab: TabId, origin: &str, ask: Ask) -> Option<Decision> {
    let scope = shell.scope_of(tab)?;
    shell
        .browser()
        .page_services()
        .grants()
        .decision(scope, &Requester::origin(origin), ask)
}

fn asked_label(prompting: &PermissionPrompt) -> Label {
    let asks = &prompting.asks;
    let has = |ask| asks.contains(&ask);
    match (has(Ask::Screen), has(Ask::Camera), has(Ask::Microphone)) {
        (true, _, _) => return tr!("browser.permission.screen"),
        (false, true, true) => return tr!("browser.permission.camera-microphone"),
        (false, true, false) => return tr!("browser.permission.camera"),
        (false, false, true) => return tr!("browser.permission.microphone"),
        (false, false, false) => {}
    }
    let kind = match &prompting.request {
        Asking::Content(request) => request.kind(),
        Asking::Media(_) => None,
    };
    match kind {
        Some(PermissionKind::Geolocation) => tr!("browser.permission.location"),
        Some(PermissionKind::DesktopNotification) => tr!("browser.permission.notifications"),
        Some(PermissionKind::MediaKeySystemAccess) => tr!("browser.permission.protected-media"),
        Some(PermissionKind::StorageAccess) => tr!(
            "browser.permission.storage-access",
            site = &Requester::origin(prompting.requester.origin.clone()).host()
        ),
        _ => tr!("browser.permission.other"),
    }
}

fn asked_symbol(prompting: &PermissionPrompt) -> MaterialSymbol {
    match prompting.asks.first() {
        Some(Ask::Screen) => MaterialSymbol::SCREEN_SHARE,
        Some(Ask::Camera) => MaterialSymbol::VIDEOCAM,
        Some(Ask::Microphone) => MaterialSymbol::MIC,
        Some(Ask::Location) => MaterialSymbol::LOCATION_ON,
        Some(Ask::Notifications) => MaterialSymbol::NOTIFICATIONS,
        Some(Ask::ProtectedMedia) => MaterialSymbol::LOCK,
        Some(Ask::StorageAccess) | None => MaterialSymbol::COOKIE,
    }
}

pub(crate) fn prompt(tab_requests: &TabRequests, tab: TabId) -> Option<Element> {
    let prompting = tab_requests.prompts.front()?;
    let site = match &prompting.requester.top_site {
        Some(top) => Requester::origin(top.clone()).host(),
        None => prompting.site.clone(),
    };
    let card = container(
        column! {
            row! {
                text(tr!("browser.permission.title", site = &site))
                    .label()
                    .semibold()
                    .grow(1.0)
                    .min_width(0.0);
                glyph_button(MaterialSymbol::CLOSE, 16.0)
                    .on_press(move |shell: &mut Shell| shell.answer_prompt(tab, None));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                icon(asked_symbol(prompting)).extent(Length::px(18.0)).color(palette().text_muted);
                text(asked_label(prompting)).grow(1.0).min_width(0.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                spacer().grow(1.0);
                button(text(tr!("browser.permission.block")))
                    .secondary()
                    .small()
                    .focusless()
                    .on_press(move |shell: &mut Shell| shell.answer_prompt(tab, Some(Decision::Deny)));
                button(text(tr!("browser.permission.allow")))
                    .primary()
                    .small()
                    .focusless()
                    .on_press(move |shell: &mut Shell| shell.answer_prompt(tab, Some(Decision::Allow)));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .border(1.0, palette().outline.with_alpha(0.6))
    .radius(Corner::Panel)
    .width(WIDTH);
    Some(card.build().keyed(("rikka-browser-prompt", tab)))
}
