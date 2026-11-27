use std::future::Future;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use crate::channel::Link;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivacySetting {
    NetworkPrediction,
    SafeBrowsing,
    SpellingService,
    TranslationService,
    PasswordSaving,
    AutofillAddresses,
    AutofillCards,
    ThirdPartyCookies,
    HyperlinkAuditing,
    DoNotTrack,
}

impl PrivacySetting {
    pub const ALL: [PrivacySetting; 10] = [
        PrivacySetting::NetworkPrediction,
        PrivacySetting::SafeBrowsing,
        PrivacySetting::SpellingService,
        PrivacySetting::TranslationService,
        PrivacySetting::PasswordSaving,
        PrivacySetting::AutofillAddresses,
        PrivacySetting::AutofillCards,
        PrivacySetting::ThirdPartyCookies,
        PrivacySetting::HyperlinkAuditing,
        PrivacySetting::DoNotTrack,
    ];

    fn path(self) -> &'static str {
        match self {
            PrivacySetting::NetworkPrediction => "privacy.network.networkPredictionEnabled",
            PrivacySetting::SafeBrowsing => "privacy.services.safeBrowsingEnabled",
            PrivacySetting::SpellingService => "privacy.services.spellingServiceEnabled",
            PrivacySetting::TranslationService => "privacy.services.translationServiceEnabled",
            PrivacySetting::PasswordSaving => "privacy.services.passwordSavingEnabled",
            PrivacySetting::AutofillAddresses => "privacy.services.autofillAddressEnabled",
            PrivacySetting::AutofillCards => "privacy.services.autofillCreditCardEnabled",
            PrivacySetting::ThirdPartyCookies => "privacy.websites.thirdPartyCookiesAllowed",
            PrivacySetting::HyperlinkAuditing => "privacy.websites.hyperlinkAuditingEnabled",
            PrivacySetting::DoNotTrack => "privacy.websites.doNotTrackEnabled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeSettingValue {
    pub value: bool,
    #[serde(rename = "levelOfControl")]
    level: Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Control {
    NotControllable,
    ControlledByOtherExtensions,
    ControllableByThisExtension,
    ControlledByThisExtension,
}

impl ChromeSettingValue {
    #[must_use]
    pub fn is_changeable(&self) -> bool {
        matches!(
            self.level,
            Control::ControllableByThisExtension | Control::ControlledByThisExtension
        )
    }
}

#[derive(Clone, Debug)]
pub struct Privacy {
    link: Arc<Link>,
}

impl Privacy {
    pub(crate) fn new(link: Arc<Link>) -> Privacy {
        Privacy { link }
    }

    pub fn get(
        &self,
        setting: PrivacySetting,
    ) -> impl Future<Output = Result<ChromeSettingValue>> + Send + use<> {
        self.link
            .call(&format!("{}.get", setting.path()), vec![json!({})])
            .decoded()
    }

    pub fn set(
        &self,
        setting: PrivacySetting,
        value: bool,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                &format!("{}.set", setting.path()),
                vec![json!({ "value": value })],
            )
            .done()
    }

    pub fn reset(
        &self,
        setting: PrivacySetting,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(&format!("{}.clear", setting.path()), vec![json!({})])
            .done()
    }
}
