use browser_engine_settings::{EngineSettings, Permission};
use rikka_web::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Endpoint, ManualProxy, PermissionDefault,
    Proxy, Spellcheck, TrackingProtection,
};
use serde_json::json;

#[test]
fn stored_document_keeps_its_names() {
    let document = json!({
        "tracking_protection": "strict",
        "cookies": "reject-trackers",
        "dns_over_https": {"preferred": "https://dns.quad9.net/dns-query"},
        "proxy": {"manual": {
            "http": {"host": "proxy.test", "port": 3128},
            "https": null,
            "socks": null,
            "socks_resolves_names": false,
            "bypass": ["localhost"]
        }},
        "page_languages": ["ja", "en"],
        "autoplay": "block-all",
        "color_scheme": "dark",
        "spellcheck": "everywhere",
        "dictionaries": ["en-US"],
        "user_style": "a { color: red }",
        "permissions": {
            "location": "block",
            "camera": "ask",
            "microphone": "ask",
            "notifications": "block",
            "protected_media": "ask",
            "screen_sharing": "block"
        }
    });
    let expected = EngineSettings {
        tracking_protection: TrackingProtection::Strict,
        cookies: CookiePolicy::RejectTrackers,
        dns_over_https: DnsOverHttps::Preferred("https://dns.quad9.net/dns-query".into()),
        proxy: Proxy::Manual(ManualProxy {
            http: Some(Endpoint::new("proxy.test", 3128)),
            bypass: vec!["localhost".into()],
            ..ManualProxy::default()
        }),
        page_languages: vec!["ja".into(), "en".into()],
        autoplay: Autoplay::BlockAll,
        color_scheme: ColorScheme::Dark,
        spellcheck: Spellcheck::Everywhere,
        dictionaries: vec!["en-US".into()],
        user_style: "a { color: red }".into(),
        permissions: EngineSettings::default()
            .permissions
            .with(Permission::Location, PermissionDefault::Block)
            .with(Permission::Notifications, PermissionDefault::Block)
            .with(Permission::ScreenSharing, PermissionDefault::Block),
    };
    let decoded: EngineSettings = serde_json::from_value(document.clone()).expect("a document");
    assert_eq!(decoded, expected);
    assert_eq!(serde_json::to_value(&expected).expect("encodes"), document);
}
