use std::time::Duration;

use rikka_gecko_protocol::{EmbedderMessage, Target};
use rikka_types::SCROLL_LINE;
use rikka_ui::Curve;
use rikka_ui::motion::SCROLL;
use rikka_web::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Endpoint, Locales, ManualProxy,
    PermissionDefault, Preferences, Proxy, Runtime, RuntimeEvent, SitePermission, Spellcheck,
    TrackingProtection,
};
use serde_json::json;

use crate::fake::FakeGecko;
use crate::support::next_matching;

#[test]
fn runtime_commands_wait_for_hello() {
    let builder = Runtime::builder()
        .preferences(Preferences::new().cookies(CookiePolicy::RejectAll))
        .locales(Locales::new(["ja-JP"]).pages(["ja-JP", "ja", "en"]));
    let (runtime, mut gecko) = FakeGecko::start_with(builder);
    let mut events = runtime.events().expect("the first to ask takes the events");

    let early = gecko.quiet_for(Duration::from_millis(300));
    assert!(
        matches!(early.as_slice(), [EmbedderMessage::Listen(listen)]
            if listen.target == Target::Runtime
            && listen.events.iter().any(|event| event == "Rikka:Notification:Show")),
        "only the runtime's listen goes out before hello: {early:?}"
    );

    gecko.hello();
    let prefs = gecko.next_event("GeckoView:SetDefaultPrefs");
    let prefs = prefs.data.expect("preferences");
    assert_eq!(prefs["network.cookie.cookieBehavior"], json!(2));
    assert_eq!(prefs["browser.privatebrowsing.autostart"], json!(true));
    let Curve::Spring(spring) = SCROLL else {
        panic!("the toolkit scrolls on a spring");
    };
    assert_eq!(prefs["general.smoothScroll.rikka.enabled"], json!(true));
    assert_eq!(
        prefs["general.smoothScroll.rikka.stiffness"],
        json!(spring.stiffness.to_string())
    );
    assert_eq!(
        prefs["general.smoothScroll.rikka.notch"],
        json!(SCROLL_LINE.to_string())
    );
    let locale = gecko.next_event("GeckoView:SetLocale");
    let locale = locale.data.expect("a locale");
    assert_eq!(locale["requestedLocales"], json!(["ja-JP"]));
    assert_eq!(locale["acceptLanguages"], json!("ja-JP, ja, en"));

    let version = next_matching(&mut events, |event| match event {
        RuntimeEvent::Started { version } => Some(version),
        _ => None,
    });
    assert_eq!(version, "158.0");
    assert_eq!(runtime.version().as_deref(), Some("158.0"));
}

#[test]
fn rejects_mismatched_protocol() {
    let (runtime, mut gecko) = FakeGecko::start();
    let mut events = runtime.events().expect("the first to ask takes the events");
    gecko.send(&rikka_gecko_protocol::GeckoMessage::Hello(
        rikka_gecko_protocol::message::Hello {
            protocol: 99,
            version: "1.0".into(),
        },
    ));
    let protocol = next_matching(&mut events, |event| match event {
        RuntimeEvent::Incompatible { protocol } => Some(protocol),
        _ => None,
    });
    assert_eq!(protocol, 99);
    assert!(!runtime.is_running());
}

#[test]
fn preconnect_uses_runtime_partition() {
    let (runtime, mut gecko) = FakeGecko::started();
    runtime.preconnect("https://example.com");
    let event = gecko.next_event("Rikka:Network:Preconnect");
    assert_eq!(
        event.data.expect("an address"),
        json!({ "uri": "https://example.com", "private": runtime.is_ephemeral() })
    );
}

#[test]
fn preferences_reach_engine_as_gecko_prefs() {
    let (runtime, mut gecko) = FakeGecko::started();
    gecko.next_event("GeckoView:SetDefaultPrefs");
    let manual = ManualProxy {
        http: Some(Endpoint::new("proxy.test", 3128)),
        https: None,
        socks: Some(Endpoint::new("socks.test", 1080)),
        socks_resolves_names: true,
        bypass: vec!["localhost".into(), "10.0.0.0/8".into()],
    };
    let cases = [
        (
            Preferences::new().tracking_protection(TrackingProtection::Strict),
            json!({"browser.contentblocking.category": "strict",
                   "privacy.trackingprotection.socialtracking.enabled": true,
                   "privacy.query_stripping.enabled": true}),
        ),
        (
            Preferences::new().tracking_protection(TrackingProtection::Off),
            json!({"browser.contentblocking.category": "custom",
                   "privacy.trackingprotection.fingerprinting.enabled": false}),
        ),
        (
            Preferences::new().cookies(CookiePolicy::RejectTrackers),
            json!({"network.cookie.cookieBehavior": 4,
                   "network.cookie.cookieBehavior.pbmode": 4}),
        ),
        (
            Preferences::new().dns_over_https(DnsOverHttps::Only("https://dns.test/q".into())),
            json!({"network.trr.mode": 3, "network.trr.uri": "https://dns.test/q"}),
        ),
        (
            Preferences::new().dns_over_https(DnsOverHttps::Off),
            json!({"network.trr.mode": 5}),
        ),
        (
            Preferences::new().proxy(Proxy::Script("https://pac.test/p.pac".into())),
            json!({"network.proxy.type": 2,
                   "network.proxy.autoconfig_url": "https://pac.test/p.pac"}),
        ),
        (
            Preferences::new().proxy(Proxy::Manual(manual)),
            json!({"network.proxy.type": 1, "network.proxy.http": "proxy.test",
                   "network.proxy.http_port": 3128, "network.proxy.ssl": "",
                   "network.proxy.ssl_port": 0, "network.proxy.socks": "socks.test",
                   "network.proxy.socks_port": 1080, "network.proxy.socks_remote_dns": true,
                   "network.proxy.no_proxies_on": "localhost, 10.0.0.0/8"}),
        ),
        (
            Preferences::new()
                .spellcheck(Spellcheck::Everywhere)
                .dictionaries(["en-US", "ja"]),
            json!({"layout.spellcheckDefault": 2, "spellchecker.dictionary": "en-US,ja"}),
        ),
        (
            Preferences::new().autoplay(Autoplay::BlockAll),
            json!({"media.autoplay.default": 5}),
        ),
        (
            Preferences::new().color_scheme(ColorScheme::Dark),
            json!({"layout.css.prefers-color-scheme.content-override": 0}),
        ),
        (
            Preferences::new()
                .permission_default(SitePermission::Location, PermissionDefault::Block)
                .permission_default(SitePermission::Notifications, PermissionDefault::Ask),
            json!({"permissions.default.geo": 2,
                   "permissions.default.desktop-notification": 0}),
        ),
    ];
    for (preferences, expected) in cases {
        runtime.set_preferences(&preferences);
        let sent = gecko
            .next_event("GeckoView:SetDefaultPrefs")
            .data
            .expect("preferences");
        for (name, value) in expected.as_object().expect("expected prefs") {
            assert_eq!(&sent[name], value, "{name}");
        }
    }
}

#[test]
fn locales_change_while_running() {
    let (runtime, mut gecko) = FakeGecko::started();
    runtime.set_locales(&Locales::new(["de-DE"]).pages(["de", "en-US"]));
    let event = gecko.next_event("GeckoView:SetLocale");
    assert_eq!(
        event.data.expect("a locale"),
        json!({"requestedLocales": ["de-DE"], "acceptLanguages": "de, en-US"})
    );
}
