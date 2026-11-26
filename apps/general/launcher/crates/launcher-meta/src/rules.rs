use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::platform::Platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    Demo,
    CustomResolution,
    QuickPlaysSupport,
    QuickPlaySingleplayer,
    QuickPlayMultiplayer,
    QuickPlayRealms,
}

impl Feature {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Feature::Demo => "is_demo_user",
            Feature::CustomResolution => "has_custom_resolution",
            Feature::QuickPlaysSupport => "has_quick_plays_support",
            Feature::QuickPlaySingleplayer => "is_quick_play_singleplayer",
            Feature::QuickPlayMultiplayer => "is_quick_play_multiplayer",
            Feature::QuickPlayRealms => "is_quick_play_realms",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features(BTreeSet<Feature>);

impl Features {
    #[must_use]
    pub fn with(mut self, feature: Feature) -> Features {
        self.0.insert(feature);
        self
    }

    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.0.iter().any(|feature| feature.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub platform: Platform,
    pub features: Features,
}

impl Environment {
    #[must_use]
    pub fn new(platform: Platform) -> Environment {
        Environment {
            platform,
            features: Features::default(),
        }
    }

    #[must_use]
    pub fn with(mut self, feature: Feature) -> Environment {
        self.features = self.features.with(feature);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Disallow,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Rule {
    pub action: Action,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}

impl Rule {
    fn applies(&self, environment: &Environment) -> bool {
        let os = self.os.as_ref().is_none_or(|os| {
            os.name
                .as_deref()
                .is_none_or(|name| name == environment.platform.os.name())
                && os
                    .arch
                    .as_deref()
                    .is_none_or(|arch| arch == environment.platform.arch.name())
        });
        os && self
            .features
            .iter()
            .all(|(name, wanted)| environment.features.has(name) == *wanted)
    }
}

#[must_use]
pub fn allowed(rules: &[Rule], environment: &Environment) -> bool {
    if rules.is_empty() {
        return true;
    }
    rules
        .iter()
        .rev()
        .find(|rule| rule.applies(environment))
        .is_some_and(|rule| rule.action == Action::Allow)
}
