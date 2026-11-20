use browser_engine_settings::{EngineSettings, Permission};
use rikka_web::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Endpoint, Locales, ManualProxy,
    PermissionDefault, Proxy, SessionSettings, Spellcheck, TrackingProtection, UserContent,
};

type Change = Box<dyn Fn(&mut EngineSettings)>;

#[test]
fn every_setting_reaches_engine_preferences() {
    let base = EngineSettings::default();
    let changes: Vec<(&str, Change)> = vec![
        (
            "tracking protection",
            Box::new(|settings| settings.tracking_protection = TrackingProtection::Strict),
        ),
        (
            "cookies",
            Box::new(|settings| settings.cookies = CookiePolicy::RejectAll),
        ),
        (
            "dns over https",
            Box::new(|settings| {
                settings.dns_over_https = DnsOverHttps::Only("https://dns.test/q".into());
            }),
        ),
        (
            "proxy",
            Box::new(|settings| {
                settings.proxy = Proxy::Manual(ManualProxy {
                    http: Some(Endpoint::new("proxy.test", 8080)),
                    ..ManualProxy::default()
                });
            }),
        ),
        (
            "autoplay",
            Box::new(|settings| settings.autoplay = Autoplay::Allowed),
        ),
        (
            "colour scheme",
            Box::new(|settings| settings.color_scheme = ColorScheme::Dark),
        ),
        (
            "spellcheck",
            Box::new(|settings| settings.spellcheck = Spellcheck::Off),
        ),
        (
            "dictionaries",
            Box::new(|settings| settings.dictionaries = vec!["en-US".into()]),
        ),
        (
            "camera default",
            Box::new(|settings| {
                settings.permissions = settings
                    .permissions
                    .with(Permission::Camera, PermissionDefault::Block);
            }),
        ),
        (
            "location default",
            Box::new(|settings| {
                settings.permissions = settings
                    .permissions
                    .with(Permission::Location, PermissionDefault::Block);
            }),
        ),
    ];
    for (name, change) in changes {
        let mut changed = base.clone();
        change(&mut changed);
        assert_ne!(
            changed.preferences(),
            base.preferences(),
            "{name} changes the engine preferences"
        );
    }
}

#[test]
fn screen_sharing_default_stays_with_browser() {
    let base = EngineSettings::default();
    let blocked = EngineSettings {
        permissions: base
            .permissions
            .with(Permission::ScreenSharing, PermissionDefault::Block),
        ..base.clone()
    };
    assert_eq!(blocked.preferences(), base.preferences());
    assert_eq!(
        blocked.permissions.get(Permission::ScreenSharing),
        PermissionDefault::Block
    );
}

#[test]
fn page_languages_default_to_interface() {
    let settings = EngineSettings::default();
    assert_eq!(settings.locales("ja-JP"), Locales::new(["ja-JP"]));
    let chosen = EngineSettings {
        page_languages: vec!["en-GB".into(), "ja".into()],
        ..EngineSettings::default()
    };
    assert_eq!(
        chosen.locales("ja-JP"),
        Locales::new(["ja-JP"]).pages(["en-GB", "ja"])
    );
}

#[test]
fn user_style_is_added_and_removed() {
    let styled = EngineSettings {
        user_style: "body { font-size: 120% }".into(),
        ..EngineSettings::default()
    };
    let dressed = styled.dress(UserContent::new());
    assert!(!dressed.is_empty());
    let plain = EngineSettings {
        user_style: "  \n".into(),
        ..EngineSettings::default()
    };
    assert!(plain.dress(dressed).is_empty());
}

#[test]
fn sessions_follow_tracking_protection() {
    let cases = [
        (TrackingProtection::Standard, true),
        (TrackingProtection::Strict, true),
        (TrackingProtection::Off, false),
    ];
    for (level, protected) in cases {
        let settings = EngineSettings {
            tracking_protection: level,
            ..EngineSettings::default()
        };
        assert_eq!(
            settings.session(SessionSettings::new()),
            SessionSettings::new().tracking_protection(protected),
            "{level:?}"
        );
    }
}
