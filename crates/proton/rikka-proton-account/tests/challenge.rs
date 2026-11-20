use std::time::Duration;

use crate::support::fixture::{PASSWORD, Server};
use rikka_proton_account::{Challenge, DeviceProfile, FieldActivity, FieldKey};
use rikka_proton_api::AppVersion;
use rikka_test_proton::response;
use serde_json::{Value, json};

fn challenge() -> Challenge {
    let version = AppVersion::parse("cli-pass@2.4.1").unwrap();

    Challenge::new(
        &version,
        DeviceProfile::collect().dark_mode(true).font_scale(1.15),
    )
}

#[test]
fn device_frame_named_after_product() {
    let payload = challenge().device_payload();
    let frame = &payload["pass-cli-v4-challenge-0"];

    assert_eq!(payload.as_object().map(|frames| frames.len()), Some(1));
    assert_eq!(frame["v"], "2.0.7");
    assert_eq!(frame["isDarkmodeOn"], true);
    assert_eq!(frame["preferredContentSize"], "1.15");
    assert_eq!(frame["isJailbreak"], false);
    for field in [
        "appLang",
        "timezone",
        "deviceName",
        "regionCode",
        "timezoneOffset",
        "storageCapacity",
        "keyboards",
    ] {
        assert!(frame.get(field).is_some(), "{field}");
    }
    assert!(frame["deviceName"].is_i64());
    assert!(frame["storageCapacity"].is_f64());
    assert!(frame.get("frame").is_none());
}

#[test]
fn field_activity_becomes_frames() {
    let mut username = FieldActivity::new();
    username.record_focus(Duration::from_secs(4));
    username.record_click();
    username.record_input("ab");
    username.record_key_press(FieldKey::Backspace);
    username.record_paste("pasted text");
    let mut recovery = FieldActivity::new();
    recovery.record_copy("copied text");

    let login = challenge().login_payload(Some(&username));
    let frame = &login["pass-cli-v4-challenge-0"];
    assert_eq!(frame["frame"], json!({"name": "username"}));
    assert_eq!(frame["timeUsername"], json!([4]));
    assert_eq!(frame["clickUsername"], 1);
    assert_eq!(
        frame["keydownUsername"],
        json!(["a", "b", "Backspace", "Paste"])
    );
    assert_eq!(frame["pasteUsername"], json!(["pasted text"]));
    assert_eq!(frame["copyUsername"], json!([]));

    let signup = challenge().signup_payload(&username, &recovery);
    assert_eq!(
        signup["pass-cli-v4-challenge-0"]["frame"],
        json!({"name": "username"})
    );
    let second = &signup["pass-cli-v4-challenge-1"];
    assert_eq!(second["frame"], json!({"name": "recovery"}));
    assert_eq!(second["copyRecovery"], json!(["copied text"]));
    assert_eq!(second["keydownRecovery"], json!(["Copy"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn login_sends_collected_payload_by_default() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|server, request| match request.route() {
            ("POST", "/auth/v4") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "UID": "uid-value",
                    "UserID": "user-value",
                    "AccessToken": "access-value",
                    "RefreshToken": "refresh-value",
                    "Scopes": ["self"],
                    "ServerProof": server.prove(&request.json()),
                    "PasswordMode": 1
                })
                .to_string(),
            )),
            _ => None,
        })
        .await;

    account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();
    let collected = server.last_body("POST", "/auth/v4").unwrap();
    assert!(collected["Payload"]["pass-linux-v4-challenge-0"]["v"].is_string());

    let session = server.last_body("POST", "/auth/v4/sessions").unwrap();
    assert!(session["Payload"]["pass-linux-v4-challenge-0"].is_object());

    account
        .login()
        .username("tester")
        .password(PASSWORD)
        .payload(json!({"custom": "frame"}))
        .execute()
        .await
        .unwrap();
    let given = server.last_body("POST", "/auth/v4").unwrap();
    assert_eq!(given["Payload"], json!({"custom": "frame"}));
    assert_ne!(given["Payload"], Value::Null);
}
