use std::collections::HashMap;

use browser_engine_settings::{Permission as Preset, PermissionDefaults};
use browser_pages::Origin;
use browser_store::{Decision, OriginDecision, Permission};
use rikka_web::{ContentPermission, MediaRequest, PermissionDefault, PermissionKind};

use crate::{ProfileId, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ask {
    Location,
    Camera,
    Microphone,
    Screen,
    Notifications,
    ProtectedMedia,
    StorageAccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Requester {
    pub origin: String,
    pub top_site: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Profile(ProfileId),
    Window(WindowId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    Allow,
    Deny,
    Prompt(Vec<Ask>),
}

pub const SITE_ASKS: [Ask; 4] = [
    Ask::Location,
    Ask::Camera,
    Ask::Microphone,
    Ask::Notifications,
];

const SCREENS: [&str; 4] = ["screen", "window", "browser", "application"];

impl Ask {
    const REMEMBERED: [Ask; 6] = [
        Ask::Location,
        Ask::Camera,
        Ask::Microphone,
        Ask::Notifications,
        Ask::ProtectedMedia,
        Ask::StorageAccess,
    ];

    pub fn stored(self) -> Option<Permission> {
        match self {
            Ask::Location => Some(Permission::Geolocation),
            Ask::Camera => Some(Permission::Camera),
            Ask::Microphone => Some(Permission::Microphone),
            Ask::Notifications => Some(Permission::Notifications),
            Ask::ProtectedMedia => Some(Permission::MediaKeySystem),
            Ask::StorageAccess => Some(Permission::StorageAccess),
            Ask::Screen => None,
        }
    }

    pub fn from_stored(permission: Permission) -> Option<Ask> {
        Ask::REMEMBERED
            .into_iter()
            .find(|ask| ask.stored() == Some(permission))
    }

    pub fn preset(self) -> Option<Preset> {
        Some(match self {
            Ask::Location => Preset::Location,
            Ask::Camera => Preset::Camera,
            Ask::Microphone => Preset::Microphone,
            Ask::Screen => Preset::ScreenSharing,
            Ask::Notifications => Preset::Notifications,
            Ask::ProtectedMedia => Preset::ProtectedMedia,
            Ask::StorageAccess => return None,
        })
    }

    pub fn preset_decision(self, defaults: &PermissionDefaults) -> Option<Decision> {
        match defaults.get(self.preset()?) {
            PermissionDefault::Ask => None,
            PermissionDefault::Block => Some(Decision::Deny),
        }
    }

    pub fn engine_name(self, requester: &Requester) -> Option<String> {
        Some(match self {
            Ask::Location => "geolocation".to_owned(),
            Ask::Camera => "camera".to_owned(),
            Ask::Microphone => "microphone".to_owned(),
            Ask::Notifications => "desktop-notification".to_owned(),
            Ask::ProtectedMedia => "media-key-system-access".to_owned(),
            Ask::StorageAccess => format!("3rdPartyStorage^{}", requester.origin),
            Ask::Screen => return None,
        })
    }
}

pub fn content_asks(permission: &ContentPermission) -> Option<Vec<Ask>> {
    let ask = match permission.kind()? {
        PermissionKind::Geolocation => Ask::Location,
        PermissionKind::DesktopNotification => Ask::Notifications,
        PermissionKind::MediaKeySystemAccess => Ask::ProtectedMedia,
        PermissionKind::StorageAccess => Ask::StorageAccess,
        PermissionKind::PersistentStorage
        | PermissionKind::Xr
        | PermissionKind::AutoplayInaudible
        | PermissionKind::AutoplayAudible
        | PermissionKind::Tracking
        | PermissionKind::LocalDeviceAccess
        | PermissionKind::LocalNetworkAccess => return None,
    };
    Some(vec![ask])
}

pub fn media_asks(request: &MediaRequest) -> Option<Vec<Ask>> {
    let screen = request
        .video
        .iter()
        .any(|source| SCREENS.contains(&source.media_source.as_str()));
    let camera = request
        .video
        .iter()
        .any(|source| !SCREENS.contains(&source.media_source.as_str()));
    let asks: Vec<Ask> = [
        (screen, Ask::Screen),
        (camera && !screen, Ask::Camera),
        (!request.audio.is_empty(), Ask::Microphone),
    ]
    .into_iter()
    .filter_map(|(requested, ask)| requested.then_some(ask))
    .collect();
    (!asks.is_empty()).then_some(asks)
}

pub fn plan(asks: Option<Vec<Ask>>, decided: impl Fn(Ask) -> Option<Decision>) -> Plan {
    let Some(asks) = asks else {
        return Plan::Deny;
    };
    let decisions: Vec<Option<Decision>> = asks.iter().map(|ask| decided(*ask)).collect();
    if decisions.contains(&Some(Decision::Deny)) {
        return Plan::Deny;
    }
    if decisions
        .iter()
        .all(|decision| *decision == Some(Decision::Allow))
    {
        return Plan::Allow;
    }
    Plan::Prompt(asks)
}

impl Requester {
    pub fn origin(origin: impl Into<String>) -> Requester {
        Requester {
            origin: origin.into(),
            top_site: None,
        }
    }

    pub fn from_page(uri: &str) -> Option<Requester> {
        Origin::parse(uri).map(|origin| Requester::origin(origin.to_string()))
    }

    pub fn from_content(permission: &ContentPermission) -> Option<Requester> {
        let page = Origin::parse(&permission.uri)?.to_string();
        match permission.storage_origin() {
            Some(requesting) if permission.kind() == Some(PermissionKind::StorageAccess) => {
                Some(Requester {
                    origin: Origin::parse(requesting)
                        .map_or_else(|| requesting.to_owned(), |origin| origin.to_string()),
                    top_site: Some(page),
                })
            }
            _ => Some(Requester::origin(page)),
        }
    }

    pub fn host(&self) -> String {
        Origin::parse(&self.origin).map_or_else(|| self.origin.clone(), |parsed| parsed.host)
    }

    pub fn engine_uri(&self) -> &str {
        self.top_site.as_deref().unwrap_or(&self.origin)
    }
}

#[derive(Debug, Default)]
pub struct Grants {
    decided: HashMap<Scope, HashMap<(Requester, Ask), Decision>>,
}

impl Grants {
    pub fn load(&mut self, profile: ProfileId, decisions: Vec<OriginDecision>) {
        let stored = self.decided.entry(Scope::Profile(profile)).or_default();
        for OriginDecision {
            origin,
            top_site,
            permission,
            decision,
        } in decisions
        {
            if let Some(ask) = Ask::from_stored(permission) {
                stored.insert((Requester { origin, top_site }, ask), decision);
            }
        }
    }

    pub fn decision(&self, scope: Scope, requester: &Requester, ask: Ask) -> Option<Decision> {
        self.decided
            .get(&scope)?
            .get(&(requester.clone(), ask))
            .copied()
    }

    pub fn decided_in(&self, scope: Scope) -> Vec<(Requester, Ask, Decision)> {
        self.decided
            .get(&scope)
            .into_iter()
            .flatten()
            .map(|((requester, ask), decision)| (requester.clone(), *ask, *decision))
            .collect()
    }

    pub fn remember(
        &mut self,
        scope: Scope,
        requester: &Requester,
        ask: Ask,
        decision: Option<Decision>,
    ) -> bool {
        if ask.stored().is_none() {
            return false;
        }
        let decisions = self.decided.entry(scope).or_default();
        let key = (requester.clone(), ask);
        let before = match decision {
            Some(decision) => decisions.insert(key, decision),
            None => decisions.remove(&key),
        };
        before != decision
    }

    pub fn forget_site(&mut self, scope: Scope, host: &str) -> Vec<(Requester, Ask)> {
        let Some(decisions) = self.decided.get_mut(&scope) else {
            return Vec::new();
        };
        let forgotten: Vec<(Requester, Ask)> = decisions
            .keys()
            .filter(|(requester, _)| requester.host() == host)
            .cloned()
            .collect();
        for key in &forgotten {
            decisions.remove(key);
        }
        forgotten
    }

    pub fn forget_scope(&mut self, scope: Scope) {
        self.decided.remove(&scope);
    }
}
