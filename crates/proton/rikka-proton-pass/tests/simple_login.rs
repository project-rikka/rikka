use rikka_proton_pass::ItemKind;
use serde_json::json;

use crate::support::{KeyMaterial, SHARE, revision_json, seeded};
use rikka_test_proton::{Request, response, success};

#[tokio::test(flavor = "multi_thread")]
async fn pending_aliases_become_items() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        let create = format!("/pass/v1/alias_sync/share/{SHARE}/create");
        match request.route() {
            ("GET", "/pass/v1/alias_sync/pending") => {
                let aliases = match request.query.contains("Since=next") {
                    true => json!([]),
                    false => json!([{
                        "PendingAliasID": "pending-value",
                        "AliasEmail": "shop.x1@simplelogin.com",
                        "AliasNote": "from SimpleLogin"
                    }]),
                };
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "PendingAliases": {"Aliases": aliases, "Total": 1, "LastToken": "next"}
                    })
                    .to_string(),
                ))
            }
            ("POST", path) if path == create => {
                let sent = request.json();
                let item = &sent["Items"][0]["Item"];
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "Revisions": {
                            "RevisionsData": [revision_json(
                                "alias-item",
                                1,
                                item["Content"].as_str()?,
                                item["ItemKey"].as_str()?,
                                None,
                            )],
                            "Total": 1,
                            "LastToken": null
                        }
                    })
                    .to_string(),
                ))
            }
            _ => None,
        }
    })
    .await;

    let pending = fixture.client.pending_aliases().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].note(), "from SimpleLogin");

    let created = fixture
        .client
        .create_pending_aliases(&fixture.share(), &pending)
        .await
        .unwrap();
    assert_eq!(created[0].name(), "shop.x1@simplelogin.com");
    assert_eq!(created[0].kind(), Some(ItemKind::Alias));

    let request = fixture
        .record
        .last("POST", &format!("/pass/v1/alias_sync/share/{SHARE}/create"))
        .json();
    assert_eq!(request["Items"][0]["PendingAliasID"], "pending-value");
    assert_eq!(request["Items"][0]["Item"]["ContentFormatVersion"], 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn alias_sync_and_mailbox_settings() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        let settings = json!({"Code": 1000, "Settings": {"DefaultAliasDomain": "proton.me", "DefaultMailboxID": 7}});
        match request.route() {
            ("GET", "/pass/v1/alias_sync/status") => Some(response(200,
                json!({"Code": 1000, "SyncStatus": {"Enabled": false, "PendingAliasCount": 12}})
                    .to_string(),
            )),
            ("POST", "/pass/v1/alias_sync/sync") => Some(success()),
            ("PUT", "/pass/v1/user/alias/settings/default_mailbox_id") => {
                Some(response(200, settings.to_string()))
            }
            ("GET", "/pass/v1/user/alias/mailbox") => Some(response(200,
                json!({
                    "Code": 1000,
                    "Mailboxes": [{
                        "MailboxID": 7,
                        "Email": "tester@proton.test",
                        "PendingEmail": "new@proton.test",
                        "Verified": true,
                        "IsDefault": true,
                        "AliasCount": 3
                    }]
                })
                .to_string(),
            )),
            ("DELETE", "/pass/v1/user/alias/mailbox/8") => Some(success()),
            _ => None,
        }
    })
    .await;

    let status = fixture.client.alias_sync_status().await.unwrap();
    assert!(!status.is_enabled());
    assert_eq!(status.pending_alias_count(), 12);

    fixture
        .client
        .enable_alias_sync(Some(&fixture.share()))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("POST", "/pass/v1/alias_sync/sync")
            .json(),
        json!({"DefaultShareID": SHARE})
    );

    let settings = fixture.client.set_default_alias_mailbox(7).await.unwrap();
    assert_eq!(settings.default_mailbox_id(), 7);
    assert_eq!(settings.default_alias_domain(), Some("proton.me"));

    let mailboxes = fixture.client.alias_mailboxes().await.unwrap();
    assert_eq!(mailboxes[0].pending_email(), Some("new@proton.test"));

    fixture
        .client
        .delete_alias_mailbox(8, Some(7))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .record
            .last("DELETE", "/pass/v1/user/alias/mailbox/8")
            .json(),
        json!({"TransferMailboxID": 7})
    );
}
