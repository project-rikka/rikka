#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignatureContext {
    value: String,
    critical: bool,
}

impl SignatureContext {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        SignatureContext {
            value: value.into(),
            critical: false,
        }
    }

    #[must_use]
    pub fn critical(mut self, critical: bool) -> Self {
        self.critical = critical;
        self
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.critical
    }
}

impl From<&SignatureContext> for proton_rpgp::SignatureContext {
    fn from(context: &SignatureContext) -> Self {
        proton_rpgp::SignatureContext::new(context.value.clone(), context.critical)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerificationContext {
    value: String,
    required: bool,
    required_after: Option<u64>,
}

impl VerificationContext {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        VerificationContext {
            value: value.into(),
            required: true,
            required_after: None,
        }
    }

    #[must_use]
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    #[must_use]
    pub fn required_after(mut self, unix_seconds: u64) -> Self {
        self.required_after = Some(unix_seconds);
        self
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn is_required(&self) -> bool {
        self.required
    }

    #[must_use]
    pub fn required_after_time(&self) -> Option<u64> {
        self.required_after
    }
}

impl From<&VerificationContext> for proton_rpgp::VerificationContext {
    fn from(context: &VerificationContext) -> Self {
        proton_rpgp::VerificationContext::new(
            context.value.clone(),
            context.required,
            context.required_after.map(proton_rpgp::UnixTime::new),
        )
    }
}
