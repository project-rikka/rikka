use serde::Serialize;

#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NewsSubscriptions {
    #[serde(rename = "Announcements", skip_serializing_if = "Option::is_none")]
    announcements: Option<bool>,
    #[serde(rename = "Features", skip_serializing_if = "Option::is_none")]
    features: Option<bool>,
    #[serde(rename = "Newsletter", skip_serializing_if = "Option::is_none")]
    newsletter: Option<bool>,
    #[serde(rename = "Beta", skip_serializing_if = "Option::is_none")]
    beta: Option<bool>,
    #[serde(rename = "Business", skip_serializing_if = "Option::is_none")]
    business: Option<bool>,
    #[serde(rename = "Offers", skip_serializing_if = "Option::is_none")]
    offers: Option<bool>,
    #[serde(rename = "NewEmailNotif", skip_serializing_if = "Option::is_none")]
    new_email_notifications: Option<bool>,
    #[serde(rename = "Onboarding", skip_serializing_if = "Option::is_none")]
    onboarding: Option<bool>,
    #[serde(rename = "UserSurveys", skip_serializing_if = "Option::is_none")]
    user_surveys: Option<bool>,
    #[serde(rename = "InboxNews", skip_serializing_if = "Option::is_none")]
    mail: Option<bool>,
    #[serde(rename = "VpnNews", skip_serializing_if = "Option::is_none")]
    vpn: Option<bool>,
    #[serde(rename = "DriveNews", skip_serializing_if = "Option::is_none")]
    drive: Option<bool>,
    #[serde(rename = "PassNews", skip_serializing_if = "Option::is_none")]
    pass: Option<bool>,
    #[serde(rename = "WalletNews", skip_serializing_if = "Option::is_none")]
    wallet: Option<bool>,
    #[serde(rename = "LumoNews", skip_serializing_if = "Option::is_none")]
    lumo: Option<bool>,
    #[serde(rename = "MeetNews", skip_serializing_if = "Option::is_none")]
    meet: Option<bool>,
    #[serde(rename = "InAppNotifications", skip_serializing_if = "Option::is_none")]
    in_app_notifications: Option<bool>,
}

impl NewsSubscriptions {
    pub fn new() -> NewsSubscriptions {
        NewsSubscriptions::default()
    }

    pub fn announcements(mut self, subscribed: bool) -> Self {
        self.announcements = Some(subscribed);
        self
    }

    pub fn features(mut self, subscribed: bool) -> Self {
        self.features = Some(subscribed);
        self
    }

    pub fn newsletter(mut self, subscribed: bool) -> Self {
        self.newsletter = Some(subscribed);
        self
    }

    pub fn beta(mut self, subscribed: bool) -> Self {
        self.beta = Some(subscribed);
        self
    }

    pub fn business(mut self, subscribed: bool) -> Self {
        self.business = Some(subscribed);
        self
    }

    pub fn offers(mut self, subscribed: bool) -> Self {
        self.offers = Some(subscribed);
        self
    }

    pub fn new_email_notifications(mut self, subscribed: bool) -> Self {
        self.new_email_notifications = Some(subscribed);
        self
    }

    pub fn onboarding(mut self, subscribed: bool) -> Self {
        self.onboarding = Some(subscribed);
        self
    }

    pub fn user_surveys(mut self, subscribed: bool) -> Self {
        self.user_surveys = Some(subscribed);
        self
    }

    pub fn mail(mut self, subscribed: bool) -> Self {
        self.mail = Some(subscribed);
        self
    }

    pub fn vpn(mut self, subscribed: bool) -> Self {
        self.vpn = Some(subscribed);
        self
    }

    pub fn drive(mut self, subscribed: bool) -> Self {
        self.drive = Some(subscribed);
        self
    }

    pub fn pass(mut self, subscribed: bool) -> Self {
        self.pass = Some(subscribed);
        self
    }

    pub fn wallet(mut self, subscribed: bool) -> Self {
        self.wallet = Some(subscribed);
        self
    }

    pub fn lumo(mut self, subscribed: bool) -> Self {
        self.lumo = Some(subscribed);
        self
    }

    pub fn meet(mut self, subscribed: bool) -> Self {
        self.meet = Some(subscribed);
        self
    }

    pub fn in_app_notifications(mut self, subscribed: bool) -> Self {
        self.in_app_notifications = Some(subscribed);
        self
    }
}
