use crate::support::fixture::{PASSWORD, Server};
use rikka_proton_account::{Error, NewsSubscriptions, UserSettings};
use rikka_test_proton::response;
use serde_json::{Value, json};

fn settings_response(proof: Option<String>) -> rikka_test_proton::ResponseFuture {
    let mut body = json!({
        "Code": 1000,
        "UserSettings": {"Email": {"Value": "new@proton.test", "Status": 0}, "Locale": "fr_FR"}
    });
    if let Some(proof) = proof {
        body["ServerProof"] = Value::String(proof);
    }

    response(200, body.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn scoped_change_sends_inline_proof() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|server, request| match request.route() {
            ("PUT", "/core/v4/settings/email") => {
                Some(settings_response(Some(server.prove(&request.json()))))
            }
            _ => None,
        })
        .await;

    let settings = account
        .set_recovery_email("new@proton.test")
        .password(PASSWORD)
        .totp("123456")
        .keep_password_scope()
        .execute()
        .await
        .unwrap();

    assert_eq!(
        settings.email().and_then(|email| email.value()),
        Some("new@proton.test")
    );

    let sent = server.last_body("PUT", "/core/v4/settings/email").unwrap();
    assert_eq!(sent["Email"], "new@proton.test");
    assert_eq!(sent["SRPSession"], "srp-session");
    assert_eq!(sent["TwoFactorCode"], "123456");
    assert_eq!(sent["PersistPasswordScope"], true);
    assert!(sent["ClientProof"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn scoped_change_uses_granted_scope() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/settings/phone/reset") => Some(settings_response(None)),
            _ => None,
        })
        .await;

    account
        .set_recovery_phone_reset(true)
        .execute()
        .await
        .unwrap();

    let sent = server
        .last_body("PUT", "/core/v4/settings/phone/reset")
        .unwrap();
    assert_eq!(sent, json!({"Reset": 1}));
    assert!(server.last_body("POST", "/auth/v4/info").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn scoped_change_rejects_invalid_server_proof() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/settings/2fa") => {
                Some(settings_response(Some("bm90LWEtcHJvb2Y=".to_owned())))
            }
            _ => None,
        })
        .await;

    let error = account
        .disable_second_factor()
        .password(PASSWORD)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::Pgp(rikka_proton_pgp::Error::ServerProof)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn enabling_totp_returns_recovery_codes() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|server, request| match request.route() {
            ("POST", "/core/v4/settings/2fa/totp") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "ServerProof": server.prove(&request.json()),
                    "TwoFactorRecoveryCodes": ["aaaa1111", "bbbb2222"],
                    "UserSettings": {"2FA": {"Enabled": 1, "Allowed": 3}}
                })
                .to_string(),
            )),
            _ => None,
        })
        .await;

    let codes = account
        .enable_totp("203941")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(codes.codes().len(), 2);
    assert_eq!(codes.codes()[0].expose(), "aaaa1111");
    assert!(
        codes
            .settings()
            .second_factor()
            .is_some_and(|factor| factor.is_enabled())
    );
    assert!(!format!("{codes:?}").contains("aaaa1111"));
    assert_eq!(
        server
            .last_body("POST", "/core/v4/settings/2fa/totp")
            .map(|sent| sent["TOTPConfirmation"].clone()),
        Some(json!("203941"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_use_their_routes() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(
            |_, request| match request.path.starts_with("/core/v4/settings/") {
                true => Some(settings_response(None)),
                false => None,
            },
        )
        .await;

    account.set_locale("fr_FR").await.unwrap();
    account.set_telemetry(false).await.unwrap();
    account.set_crash_reports(true).await.unwrap();
    account
        .set_news(&NewsSubscriptions::new().newsletter(false).pass(true))
        .await
        .unwrap();
    account.set_device_recovery(true).await.unwrap();
    account
        .verify_recovery_email("verification-token")
        .await
        .unwrap();
    account.set_fido2_credential_pin(true).await.unwrap();

    let expected = [
        (
            "PUT",
            "/core/v4/settings/flags",
            json!({"Fido2CredentialsPinOptIn": 1}),
        ),
        (
            "PUT",
            "/core/v4/settings/locale",
            json!({"Locale": "fr_FR"}),
        ),
        (
            "PUT",
            "/core/v4/settings/telemetry",
            json!({"Telemetry": 0}),
        ),
        (
            "PUT",
            "/core/v4/settings/crashreports",
            json!({"CrashReports": 1}),
        ),
        (
            "PATCH",
            "/core/v4/settings/news",
            json!({"Newsletter": false, "PassNews": true}),
        ),
        (
            "PUT",
            "/core/v4/settings/devicerecovery",
            json!({"DeviceRecovery": 1}),
        ),
        (
            "POST",
            "/core/v4/settings/email/verify",
            json!({"Token": "verification-token"}),
        ),
    ];
    for (method, path, body) in expected {
        assert_eq!(
            server.last_body(method, path),
            Some(body),
            "{method} {path}"
        );
    }
}

#[test]
fn fido2_pin_preference_read_from_flags() {
    let settings: UserSettings = serde_json::from_value(json!({
        "Flags": {"Welcomed": 1, "SupportPgpV6Keys": 0, "Fido2CredentialsPinOptIn": 1}
    }))
    .unwrap();

    assert!(settings.flags().opted_into_fido2_credential_pin());
    assert!(settings.flags().is_welcomed());
    assert!(!settings.flags().supports_v6_keys());
}
