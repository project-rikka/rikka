use browser_pages::{ErrorAction, FailureKind};
use rikka_web::ErrorCode;

#[test]
fn engine_errors_map_to_failures_and_actions() {
    let cases = [
        (
            ErrorCode::UnknownHost,
            FailureKind::HostNotFound,
            ErrorAction::Retry,
        ),
        (
            ErrorCode::NetTimeout,
            FailureKind::TimedOut,
            ErrorAction::Retry,
        ),
        (
            ErrorCode::SecurityBadCert,
            FailureKind::Tls,
            ErrorAction::Retry,
        ),
        (
            ErrorCode::SafeBrowsingPhishingUri,
            FailureKind::Blocked,
            ErrorAction::Back,
        ),
        (
            ErrorCode::FileNotFound,
            FailureKind::FileNotFound,
            ErrorAction::Back,
        ),
        (
            ErrorCode::UnknownProtocol,
            FailureKind::UnsupportedScheme,
            ErrorAction::OpenOutside,
        ),
        (
            ErrorCode::UnknownProxyHost,
            FailureKind::Proxy,
            ErrorAction::Retry,
        ),
        (ErrorCode::HttpsOnly, FailureKind::Other, ErrorAction::Retry),
    ];
    for (code, kind, action) in cases {
        assert_eq!(FailureKind::from_code(code), kind, "{code:?}");
        assert_eq!(kind.action(), action, "{kind:?}");
    }
}
