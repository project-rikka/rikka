#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Vendor {
    Anthropic,
    OpenAi,
    Xai,
    Meta,
    Antigravity,
    OpenRouter,
    Compatible,
}

impl Vendor {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Vendor::Anthropic => "Anthropic",
            Vendor::OpenAi => "OpenAI",
            Vendor::Xai => "xAI",
            Vendor::Meta => "Meta",
            Vendor::Antigravity => "Antigravity",
            Vendor::OpenRouter => "OpenRouter",
            Vendor::Compatible => "the endpoint",
        }
    }
}

impl std::fmt::Display for Vendor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}
