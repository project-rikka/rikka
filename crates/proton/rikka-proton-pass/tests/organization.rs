use rikka_proton_pass::{
    ItemShareMode, MonitorReport, OrganizationUpdate, PasswordPolicy, PauseValues, PausedUrlId,
    ShareMode,
};
use serde_json::json;

use crate::support::{KeyMaterial, seeded};
use rikka_test_proton::{Request, response, success};

fn organization(share_mode: i32) -> String {
    json!({
        "Code": 1000,
        "Organization": {
            "CanUpdate": true,
            "Settings": {"ShareMode": share_mode, "ForceLockSeconds": 600}
        }
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_updates_organization_settings() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        match request.route() {
            ("PUT", "/pass/v1/organization/settings") => Some(response(200, organization(1))),
            ("PUT", "/pass/v1/organization/settings/password_policy") => {
                Some(response(200, organization(0)))
            }
            _ => None,
        }
    })
    .await;

    let updated = fixture
        .client
        .update_organization(
            &OrganizationUpdate::new()
                .share_mode(ShareMode::InsideOrganization)
                .item_share_mode(ItemShareMode::Disabled)
                .force_lock_seconds(600),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.settings().map(|settings| settings.share_mode()),
        Some(ShareMode::InsideOrganization)
    );
    assert_eq!(
        fixture
            .record
            .last("PUT", "/pass/v1/organization/settings")
            .json(),
        json!({"ShareMode": 1, "ItemShareMode": 0, "ForceLockSeconds": 600})
    );

    fixture
        .client
        .update_password_policy(
            &PasswordPolicy::new(true, false)
                .with_random_length(Some(12), Some(64))
                .with_random_symbols(Some(true)),
        )
        .await
        .unwrap();
    let policy = fixture
        .record
        .last("PUT", "/pass/v1/organization/settings/password_policy")
        .json();
    assert_eq!(policy["RandomPasswordAllowed"], true);
    assert_eq!(policy["RandomPasswordMinLength"], 12);
    assert_eq!(policy["RandomPasswordMaxLength"], 64);
    assert_eq!(policy["RandomPasswordMustIncludeSymbols"], true);
    assert_eq!(policy["MemorablePasswordAllowed"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn member_reports_and_paused_urls() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        let entry = json!({
            "EntryID": "entry-value",
            "Url": "https://intranet.example",
            "Values": {
                "AutofillEnabled": false,
                "Autofill2faEnabled": false,
                "AutofillAutosuggestEnabled": true,
                "AutosaveEnabled": false,
                "PasskeysEnabled": true
            },
            "CreateTime": 3
        });
        match request.route() {
            ("GET", "/pass/v1/organization/report") => Some(response(200,
                json!({
                    "Code": 1000,
                    "Report": {
                        "TotalMemberCount": 1,
                        "MemberReports": [{
                            "PrimaryEmail": "member@proton.test",
                            "CustomEmailBreachCount": {"BreachEmailCount": 1, "TotalBreachCount": 4},
                            "AddressBreachCount": {"BreachEmailCount": 0, "TotalBreachCount": 0},
                            "ItemsReport": {"OwnedVaultCount": 2, "OwnedItemCount": 30},
                            "MonitorReport": {
                                "UserID": 1,
                                "OrganizationID": 2,
                                "ReusedPasswords": 5,
                                "Inactive2FA": 1,
                                "ExcludedItems": 0,
                                "WeakPasswords": 3,
                                "ReportTime": 10,
                                "ClientVersion": "web-pass@1.0.0"
                            },
                            "LastActivityTime": 20
                        }]
                    }
                })
                .to_string(),
            )),
            ("POST", "/pass/v1/organization/report/client_data") => Some(success()),
            ("GET", "/pass/v1/organization/urlpause") => Some(response(200,
                json!({"Code": 1000, "Entries": {"Entries": [entry], "Total": 1, "LastID": null}})
                    .to_string(),
            )),
            ("POST", "/pass/v1/organization/urlpause") => {
                Some(response(200, json!({"Code": 1000, "Entry": entry}).to_string()))
            }
            ("PUT" | "DELETE", "/pass/v1/organization/urlpause/entry-value") => Some(success()),
            _ => None,
        }
    })
    .await;

    let reports = fixture.client.organization_reports().await.unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].custom_email_breaches().breaches(), 4);
    assert_eq!(reports[0].items().owned_items(), 30);
    assert_eq!(
        reports[0]
            .monitor()
            .map(|monitor| monitor.counts().reused_passwords()),
        Some(5)
    );

    fixture
        .client
        .send_monitor_report(
            &MonitorReport::new()
                .with_reused_passwords(2)
                .with_weak_passwords(1)
                .with_compromised_passwords(3),
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("POST", "/pass/v1/organization/report/client_data")
            .json(),
        json!({
            "ReusedPasswords": 2,
            "Inactive2FA": 0,
            "ExcludedItems": 0,
            "WeakPasswords": 1,
            "CompromisedPasswords": 3
        })
    );

    let paused = fixture.client.paused_urls().await.unwrap();
    assert!(paused[0].values().passkeys);

    let values = PauseValues {
        autosave: true,
        ..PauseValues::default()
    };
    fixture
        .client
        .pause_url("https://intranet.example", values)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("POST", "/pass/v1/organization/urlpause")
            .json()["Url"],
        "https://intranet.example"
    );

    let entry = PausedUrlId::from("entry-value");
    fixture
        .client
        .update_paused_url(&entry, values)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("PUT", "/pass/v1/organization/urlpause/entry-value")
            .json()["AutosaveEnabled"],
        true
    );
    fixture.client.remove_paused_url(&entry).await.unwrap();
    fixture
        .record
        .last("DELETE", "/pass/v1/organization/urlpause/entry-value");
}
