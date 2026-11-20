use crate::vendor::Vendor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionsDialect {
    Compatible,
    OpenRouter,
}

impl CompletionsDialect {
    pub(crate) fn vendor(self) -> Vendor {
        match self {
            CompletionsDialect::Compatible => Vendor::Compatible,
            CompletionsDialect::OpenRouter => Vendor::OpenRouter,
        }
    }

    pub(crate) fn replays(self) -> bool {
        matches!(self, CompletionsDialect::OpenRouter)
    }

    pub(crate) fn declares_leniency(self) -> bool {
        matches!(self, CompletionsDialect::OpenRouter)
    }

    pub(crate) fn thinks_inline(self) -> bool {
        matches!(self, CompletionsDialect::Compatible)
    }
}
