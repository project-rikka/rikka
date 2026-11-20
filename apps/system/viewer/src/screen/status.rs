use rikka_app::prelude::*;

#[derive(Debug)]
pub(crate) struct StatusView {
    symbol: MaterialSymbol,
    message: Label,
}

impl StatusView {
    pub(crate) fn unsupported() -> Self {
        Self {
            symbol: MaterialSymbol::BROKEN_IMAGE,
            message: tr!("viewer.status.unsupported"),
        }
    }

    pub(crate) fn cannot_read(error: &std::io::Error) -> Self {
        Self {
            symbol: MaterialSymbol::ERROR,
            message: unreadable(error),
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            symbol: MaterialSymbol::VISIBILITY,
            message: tr!("viewer.status.empty"),
        }
    }

    pub(crate) fn stage(&self) -> Element {
        status_page(self.symbol, self.message.clone())
            .grow(1.0)
            .build()
    }
}

pub(crate) fn unreadable(error: &dyn std::error::Error) -> Label {
    let mut reason = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        reason = format!("{reason}: {cause}");
        source = cause.source();
    }
    tr!("viewer.status.unreadable", reason = reason.as_str())
}

pub(crate) fn too_large(width: u32, height: u32) -> Label {
    tr!("viewer.status.too_large", width = width, height = height)
}
