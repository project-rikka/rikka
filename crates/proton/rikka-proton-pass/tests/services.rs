use rikka_proton_pass::{
    CustomEmailId, ItemId, NotificationDisplay, NotificationId, NotificationState, ShareMode,
    VaultCreateMode,
};
use serde_json::json;

use crate::support::{ITEM, KeyMaterial, SHARE, seeded};
use rikka_test_proton::{Request, response, success};

#[tokio::test(flavor = "multi_thread")]
async fn notifications_and_policies_endpoints() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| match request.route() {
        ("GET", "/pass/v1/notification") => {
            let notifications = match request.query.contains("Since=first") {
                true => json!([]),
                false => json!([{
                    "ID": "first",
                    "NotificationKey": "pass_welcome",
                    "StartTime": 100,
                    "EndTime": 200,
                    "State": 0,
                    "Priority": 2,
                    "Content": {
                        "DisplayType": 1,
                        "Title": "Welcome",
                        "Message": "Hello",
                        "Cta": {"Text": "Open", "Type": "external_link", "Ref": "https://proton.me"}
                    }
                }]),
            };
            Some(response(
                200,
                json!({
                    "Code": 1000,
                    "Notifications": {"Notifications": notifications, "Total": 1, "LastID": "first"}
                })
                .to_string(),
            ))
        }
        ("PUT", "/pass/v1/notification/first") => Some(success()),
        ("GET", "/pass/v1/organization") => Some(response(
            200,
            json!({
                "Code": 1000,
                "Organization": {
                    "CanUpdate": false,
                    "Settings": {
                        "ShareMode": 1,
                        "ForceLockSeconds": 300,
                        "VaultCreateMode": 2,
                        "PasswordPolicy": {
                            "RandomPasswordAllowed": true,
                            "RandomPasswordMinLength": 16,
                            "MemorablePasswordAllowed": false
                        }
                    }
                }
            })
            .to_string(),
        )),
        _ => None,
    })
    .await;

    let notifications = fixture.client.notifications().await.unwrap();
    assert_eq!(notifications.len(), 1);
    let notification = &notifications[0];
    assert_eq!(notification.content().display(), NotificationDisplay::Modal);
    assert!(
        notification
            .content()
            .cta()
            .is_some_and(|cta| cta.is_external_link())
    );
    assert!(notification.is_shown_at(jiff::Timestamp::from_second(150).unwrap()));
    assert!(!notification.is_shown_at(jiff::Timestamp::from_second(250).unwrap()));

    fixture
        .client
        .set_notification_state(&NotificationId::from("first"), NotificationState::Dismissed)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("PUT", "/pass/v1/notification/first")
            .json(),
        json!({"State": 2})
    );

    let organization = fixture.client.organization().await.unwrap().unwrap();
    let settings = organization.settings().unwrap();
    assert_eq!(settings.share_mode(), ShareMode::InsideOrganization);
    assert_eq!(
        settings.vault_create_mode(),
        VaultCreateMode::AdminsAndPersonalVault
    );
    assert_eq!(settings.force_lock_seconds(), Some(300));
    assert_eq!(
        settings
            .password_policy()
            .and_then(|policy| policy.random_min_length()),
        Some(16)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn breach_and_alias_contact_endpoints() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        let email = json!({
            "CustomEmailID": "custom-value",
            "Email": "me@example.com",
            "Verified": false,
            "BreachCounter": 0,
            "Flags": 1,
            "LastCheckTime": 5
        });
        let contact = json!({
            "ID": 9,
            "Name": "Shop",
            "Blocked": true,
            "ReverseAlias": "reverse@proton.me",
            "Email": "shop@example.com",
            "CreateTime": 1
        });
        let contacts = format!("/pass/v1/share/{SHARE}/alias/{ITEM}/contact");
        let blocked = format!("/pass/v1/share/{SHARE}/alias/{ITEM}/contact/9/blocked");
        match request.route() {
            ("POST", "/pass/v1/breach/custom_email")
            | ("PUT", "/pass/v1/breach/custom_email/custom-value/verify") => Some(response(
                200,
                json!({"Code": 1000, "Email": email}).to_string(),
            )),
            ("PUT", "/pass/v1/user/monitor") => Some(response(
                200,
                json!({"Code": 1000, "Monitor": {"ProtonAddress": false, "Aliases": true}})
                    .to_string(),
            )),
            ("GET", path) if path == contacts => Some(response(
                200,
                json!({"Code": 1000, "Contacts": [contact], "Total": 1, "LastID": 0}).to_string(),
            )),
            ("PUT", path) if path == blocked => Some(response(
                200,
                json!({"Code": 1000, "Contact": contact}).to_string(),
            )),
            _ => None,
        }
    })
    .await;

    let added = fixture
        .client
        .add_custom_email("me@example.com")
        .await
        .unwrap();
    assert!(!added.is_monitored());
    assert_eq!(
        fixture
            .record
            .last("POST", "/pass/v1/breach/custom_email")
            .json(),
        json!({"Email": "me@example.com"})
    );

    fixture
        .client
        .verify_custom_email(&CustomEmailId::from("custom-value"), "123456")
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("PUT", "/pass/v1/breach/custom_email/custom-value/verify")
            .json(),
        json!({"Code": "123456"})
    );

    let monitor = fixture
        .client
        .set_breach_monitoring(Some(false), None)
        .await
        .unwrap();
    assert!(!monitor.watches_proton_addresses());
    assert!(monitor.watches_aliases());
    assert_eq!(
        fixture.record.last("PUT", "/pass/v1/user/monitor").json(),
        json!({"ProtonAddress": false})
    );

    let item = ItemId::from(ITEM);
    let contacts = fixture
        .client
        .alias_contacts(&fixture.share(), &item)
        .await
        .unwrap();
    assert_eq!(contacts[0].reverse_alias(), "reverse@proton.me");

    let blocked = fixture
        .client
        .set_alias_contact_blocked(&fixture.share(), &item, 9, true)
        .await
        .unwrap();
    assert!(blocked.is_blocked());
    assert_eq!(
        fixture
            .record
            .last(
                "PUT",
                &format!("/pass/v1/share/{SHARE}/alias/{ITEM}/contact/9/blocked")
            )
            .json(),
        json!({"Blocked": true})
    );
}
