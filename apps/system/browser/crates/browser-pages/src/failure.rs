use rikka_web::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    HostNotFound,
    ConnectionRefused,
    ConnectionReset,
    TimedOut,
    Offline,
    Tls,
    TooManyRedirects,
    Blocked,
    FileNotFound,
    UnsupportedScheme,
    Proxy,
    Content,
    Other,
}

impl FailureKind {
    #[must_use]
    pub fn from_code(code: ErrorCode) -> FailureKind {
        match code {
            ErrorCode::UnknownHost | ErrorCode::MalformedUri => FailureKind::HostNotFound,
            ErrorCode::ConnectionRefused => FailureKind::ConnectionRefused,
            ErrorCode::NetReset | ErrorCode::NetInterrupt => FailureKind::ConnectionReset,
            ErrorCode::NetTimeout => FailureKind::TimedOut,
            ErrorCode::Offline => FailureKind::Offline,
            ErrorCode::SecuritySsl | ErrorCode::SecurityBadCert | ErrorCode::BadHstsCert => {
                FailureKind::Tls
            }
            ErrorCode::RedirectLoop => FailureKind::TooManyRedirects,
            ErrorCode::PortBlocked
            | ErrorCode::LocalNetworkAccessDenied
            | ErrorCode::SafeBrowsingMalwareUri
            | ErrorCode::SafeBrowsingUnwantedUri
            | ErrorCode::SafeBrowsingHarmfulUri
            | ErrorCode::SafeBrowsingPhishingUri
            | ErrorCode::HarmfulAddonUri => FailureKind::Blocked,
            ErrorCode::FileNotFound | ErrorCode::FileAccessDenied => FailureKind::FileNotFound,
            ErrorCode::UnknownProtocol | ErrorCode::UnknownSocketType => {
                FailureKind::UnsupportedScheme
            }
            ErrorCode::ProxyConnectionRefused | ErrorCode::UnknownProxyHost => FailureKind::Proxy,
            ErrorCode::UnsafeContentType
            | ErrorCode::CorruptedContent
            | ErrorCode::ContentCrashed
            | ErrorCode::InvalidContentEncoding
            | ErrorCode::DataUriTooLong => FailureKind::Content,
            ErrorCode::Unknown | ErrorCode::HttpsOnly => FailureKind::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorAction {
    Retry,
    Back,
    OpenOutside,
}

impl FailureKind {
    #[must_use]
    pub fn action(self) -> ErrorAction {
        match self {
            FailureKind::Blocked | FailureKind::FileNotFound => ErrorAction::Back,
            FailureKind::UnsupportedScheme => ErrorAction::OpenOutside,
            FailureKind::Offline
            | FailureKind::Tls
            | FailureKind::TooManyRedirects
            | FailureKind::Content
            | FailureKind::HostNotFound
            | FailureKind::ConnectionRefused
            | FailureKind::ConnectionReset
            | FailureKind::TimedOut
            | FailureKind::Proxy
            | FailureKind::Other => ErrorAction::Retry,
        }
    }
}
