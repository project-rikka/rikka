use crate::email_preferences::EmailPreferences;

#[derive(Debug, Clone, Default)]
pub struct PinnedKeys {
    preferences: EmailPreferences,
    contact: bool,
    signature_verified: Option<bool>,
    signature_time: Option<u64>,
    problem: Option<String>,
}

impl PinnedKeys {
    #[must_use]
    pub fn none() -> PinnedKeys {
        PinnedKeys::default()
    }

    #[must_use]
    pub fn for_contact(preferences: EmailPreferences) -> PinnedKeys {
        PinnedKeys {
            preferences,
            contact: true,
            ..PinnedKeys::default()
        }
    }

    #[must_use]
    pub fn contact_without_preferences() -> PinnedKeys {
        PinnedKeys {
            contact: true,
            ..PinnedKeys::default()
        }
    }

    #[must_use]
    pub fn with_verification(mut self, verified: bool, signed_at: Option<u64>) -> PinnedKeys {
        self.signature_verified = Some(verified);
        self.signature_time = signed_at;
        self
    }

    #[must_use]
    pub fn with_problem(mut self, problem: impl Into<String>) -> PinnedKeys {
        self.problem = Some(problem.into());
        self.preferences = EmailPreferences::default();
        self
    }

    #[must_use]
    pub fn preferences(&self) -> &EmailPreferences {
        &self.preferences
    }

    #[must_use]
    pub fn is_contact(&self) -> bool {
        self.contact
    }

    #[must_use]
    pub fn signature_verified(&self) -> Option<bool> {
        self.signature_verified
    }

    #[must_use]
    pub fn signature_time(&self) -> Option<u64> {
        self.signature_time
    }

    #[must_use]
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }
}
