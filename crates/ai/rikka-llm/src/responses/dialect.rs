use crate::vendor::Vendor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsesDialect {
    OpenAi,
    Codex,
    Compatible,
}

impl ResponsesDialect {
    pub(crate) fn vendor(self) -> Vendor {
        match self {
            ResponsesDialect::Compatible => Vendor::Compatible,
            ResponsesDialect::OpenAi | ResponsesDialect::Codex => Vendor::OpenAi,
        }
    }

    pub(super) fn caches(self) -> bool {
        self == ResponsesDialect::OpenAi
    }

    pub(super) fn defers_tools(self) -> bool {
        self == ResponsesDialect::OpenAi
    }

    pub(super) fn limits(self) -> bool {
        self != ResponsesDialect::Codex
    }

    pub(super) fn is_cli_dialect(self) -> bool {
        self == ResponsesDialect::Codex
    }

    pub(super) fn streams_only(self) -> bool {
        self == ResponsesDialect::Codex
    }

    pub(super) fn is_hosted(self) -> bool {
        self != ResponsesDialect::Compatible
    }
}
