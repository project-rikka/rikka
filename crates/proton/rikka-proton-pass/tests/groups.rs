use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_pass::{EncryptionKey, EncryptionTag, Error, GroupId, Vault};
use rikka_proton_pgp::{Pgp, PrivateKey, SignatureContext};
use serde_json::{Value, json};

use crate::support::{
    Fixture, KeyMaterial, login, new_key, public_keys, revision_json, seal_base64, seeded,
    share_json,
};
use rikka_test_proton::{Request, lock_key, response, success};

const GROUP: &str = "group-value";
const GROUP_EMAIL: &str = "team@proton.test";
const INVITER: &str = "inviter@proton.test";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn token_for(
    recipient: &PrivateKey,
    signer: &PrivateKey,
    context: Option<&str>,
) -> (String, String) {
    let pgp = Pgp::new();
    let token = pgp
        .encrypt()
        .to(recipient.public_key())
        .armored_message(TOKEN)
        .unwrap();
    let mut signing = pgp.sign().with_key(signer.clone());
    if let Some(context) = context {
        signing = signing.with_context(SignatureContext::new(context).critical(true));
    }
    let signature = signing.armored_detached(TOKEN).unwrap();

    (token, signature)
}

fn group_json(group_key: &PrivateKey, token: &(String, String)) -> Value {
    json!({
        "Code": 1000,
        "Total": 1,
        "Groups": [{
            "ID": GROUP,
            "Name": "Team",
            "Address": {
                "ID": "group-address",
                "Email": GROUP_EMAIL,
                "Keys": [{
                    "ID": "group-key",
                    "Version": 3,
                    "Flags": 3,
                    "PrivateKey": lock_key(group_key, TOKEN),
                    "Token": token.0,
                    "Signature": token.1,
                    "Primary": 1,
                    "Active": 1
                }]
            },
            "Permissions": 0,
            "CreateTime": 1,
            "Flags": 0,
            "GroupVisibility": 0,
            "MemberVisibility": 0
        }]
    })
}

fn invite_json(
    token: &str,
    owner: bool,
    vault_key: &EncryptionKey,
    group_key: &PrivateKey,
    inviter: &PrivateKey,
) -> Value {
    let vault = Vault {
        name: "Team vault".to_owned(),
        description: String::new(),
        display: None,
    };
    let sealed_key = Pgp::new()
        .encrypt()
        .to(group_key.public_key())
        .signed_by(inviter.clone())
        .with_context(SignatureContext::new("pass.invite.vault.existing-user").critical(true))
        .message(vault_key.as_bytes())
        .unwrap();

    json!({
        "InviteID": format!("invite-{token}"),
        "InviterUserID": "inviter-user",
        "InviterEmail": INVITER,
        "InvitedGroupID": GROUP,
        "InvitedEmail": GROUP_EMAIL,
        "TargetType": 1,
        "TargetID": "vault-value",
        "RemindersSent": 0,
        "InviteToken": token,
        "InvitedAddressID": "group-address",
        "Keys": [{"KeyRotation": 1, "Key": BASE64_STANDARD.encode(sealed_key)}],
        "VaultData": {
            "Content": seal_base64(vault_key, vault.encode(), &EncryptionTag::VaultContent),
            "ContentKeyRotation": 1,
            "ContentFormatVersion": 1,
            "MemberCount": 4,
            "ItemCount": 2
        },
        "IsGroupOwner": owner,
        "CreateTime": 1
    })
}

fn decrypt_with_group_key(posted: &Value, group_key: &PrivateKey) -> Vec<u8> {
    let message = BASE64_STANDARD
        .decode(posted["Keys"][0]["Key"].as_str().unwrap())
        .unwrap();
    let decrypted = Pgp::new()
        .decrypt()
        .with_key(group_key.clone())
        .verified_by(group_key.public_key())
        .message(message)
        .unwrap();

    assert!(
        decrypted.signature().is_valid(),
        "{:?}",
        decrypted.signature()
    );
    decrypted.into_data()
}

#[tokio::test(flavor = "multi_thread")]
async fn group_owner_accepts_group_invite() {
    let seeds = KeyMaterial::new();
    let group_key = new_key(GROUP_EMAIL);
    let inviter = new_key(INVITER);
    let vault_key = EncryptionKey::generate();
    let groups = group_json(
        &group_key,
        &token_for(&seeds.user, &seeds.user, Some("account.key-token.address")),
    );
    let invites = json!({
        "Code": 1000,
        "Invites": {
            "Invites": [invite_json("owned-value", true, &vault_key, &group_key, &inviter)],
            "Total": 1,
            "LastID": null
        }
    });

    let fixture = {
        let inviter = inviter.clone();
        seeded(seeds, move |request: &Request| match request.route() {
            ("GET", "/core/v4/groups") => Some(response(200, groups.to_string())),
            ("GET", "/core/v4/groups/group-value/members") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "Total": 1,
                    "Members": [{
                        "ID": "member-value",
                        "Type": 0,
                        "State": 1,
                        "CreateTime": 1,
                        "GroupID": GROUP,
                        "AddressID": "address-value",
                        "Email": "tester@proton.test"
                    }]
                })
                .to_string(),
            )),
            ("GET", "/pass/v1/invite/group") => Some(response(200, invites.to_string())),
            ("GET", "/core/v4/keys/all") => Some(public_keys(&[&inviter])),
            ("POST", "/pass/v1/invite/group/owned-value") => Some(success()),
            _ => None,
        })
        .await
    };

    let members = fixture
        .client
        .group_members(&GroupId::from(GROUP))
        .await
        .unwrap();
    assert_eq!(members[0].email(), Some("tester@proton.test"));

    let pending = fixture.client.group_invites().await.unwrap();
    assert_eq!(
        pending[0].vault().map(|vault| vault.name.as_str()),
        Some("Team vault")
    );
    assert_eq!(pending[0].group_email(), GROUP_EMAIL);

    fixture
        .client
        .accept_group_invite(&pending[0])
        .await
        .unwrap();
    let posted = fixture
        .record
        .last("POST", "/pass/v1/invite/group/owned-value")
        .json();
    assert_eq!(posted["Keys"][0]["KeyRotation"], 1);
    assert_eq!(
        decrypt_with_group_key(&posted, &group_key),
        vault_key.as_bytes()
    );
}

struct AdminFixture {
    fixture: Fixture,
    group_key: PrivateKey,
    vault_key: EncryptionKey,
}

async fn admin_fixture(
    organization_signer: Option<PrivateKey>,
    context: &str,
    vault: bool,
) -> AdminFixture {
    let seeds = KeyMaterial::new();
    let organization = new_key("organization@proton.test");
    let group_key = new_key(GROUP_EMAIL);
    let inviter = new_key(INVITER);
    let vault_key = EncryptionKey::generate();

    let signer = organization_signer.unwrap_or_else(|| seeds.user.clone());
    let organization_token = token_for(&seeds.user, &signer, Some(context));
    let organization_keys = json!({
        "Code": 1000,
        "PrivateKey": lock_key(&organization, TOKEN),
        "Token": organization_token.0,
        "Signature": organization_token.1,
        "Passwordless": true
    });
    let groups = group_json(&group_key, &token_for(&organization, &organization, None));
    let mut invite = invite_json("admin-value", false, &vault_key, &group_key, &inviter);
    if !vault {
        invite["TargetType"] = json!(2);
        invite["VaultData"] = Value::Null;
    }
    let invites = json!({"Code": 1000, "Invites": {"Invites": [invite], "LastID": null}});

    let fixture = seeded(seeds, move |request: &Request| match request.route() {
        ("GET", "/core/v4/groups") => Some(response(200, groups.to_string())),
        ("GET", "/pass/v1/invite/group") => Some(response(200, invites.to_string())),
        ("GET", "/core/v4/organizations/keys") => {
            Some(response(200, organization_keys.to_string()))
        }
        ("GET", "/core/v4/keys/all") => Some(public_keys(&[&inviter])),
        ("POST", "/pass/v1/invite/group/admin-value") => Some(success()),
        _ => None,
    })
    .await;

    AdminFixture {
        fixture,
        group_key,
        vault_key,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_opens_group_key_via_organization() {
    let AdminFixture {
        fixture,
        group_key,
        vault_key,
    } = admin_fixture(None, "account.key-token.organization", true).await;

    let pending = fixture.client.group_invites().await.unwrap();
    assert_eq!(
        pending[0].vault().map(|vault| vault.name.as_str()),
        Some("Team vault")
    );
    assert!(!pending[0].is_group_owner());

    fixture
        .client
        .accept_group_invite(&pending[0])
        .await
        .unwrap();
    let posted = fixture
        .record
        .last("POST", "/pass/v1/invite/group/admin-value")
        .json();
    assert_eq!(
        decrypt_with_group_key(&posted, &group_key),
        vault_key.as_bytes()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unsigned_organization_token() {
    let cases = [
        (
            Some(new_key("stranger@proton.test")),
            "account.key-token.organization",
        ),
        (None, "account.key-token.elsewhere"),
    ];

    for (signer, context) in cases {
        let AdminFixture { fixture, .. } = admin_fixture(signer, context, false).await;

        let pending = fixture.client.group_invites().await.unwrap();
        assert!(pending[0].vault().is_none());

        let error = fixture
            .client
            .accept_group_invite(&pending[0])
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            Error::Account(rikka_proton_account::Error::UnsignedKeyToken(_))
        ));
        assert!(
            fixture
                .record
                .matching("POST", "/pass/v1/invite/group/admin-value")
                .is_empty()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn group_vault_requires_group_signature() {
    let foreign_key = new_key("stranger@proton.test");
    let group_key = new_key(GROUP_EMAIL);

    for (signer, readable) in [(group_key.clone(), true), (foreign_key, false)] {
        let seeds = KeyMaterial::new();
        let vault_key = EncryptionKey::generate();
        let item_key = EncryptionKey::generate();
        let sealed_key = Pgp::new()
            .encrypt()
            .to(seeds.address.public_key())
            .signed_by(signer)
            .message(vault_key.as_bytes())
            .unwrap();

        let mut share = share_json("group-share", &vault_key);
        share["GroupID"] = json!(GROUP);
        share["Owner"] = json!(false);
        let shares = json!({"Code": 1000, "Shares": [share]});
        let keys = json!({
            "Code": 1000,
            "ShareKeys": {
                "Total": 1,
                "Keys": [{"KeyRotation": 1, "Key": BASE64_STANDARD.encode(sealed_key), "CreateTime": 1}]
            }
        });
        let group = json!({
            "Code": 1000,
            "Group": {"ID": GROUP, "Name": "Team", "Address": {"ID": "group-address", "Email": GROUP_EMAIL}}
        });
        let items = json!({
            "Code": 1000,
            "Items": {
                "RevisionsData": [revision_json(
                    "team-item",
                    1,
                    &seal_base64(&item_key, login("Team login", "hunter9").encode(), &EncryptionTag::ItemContent),
                    &seal_base64(&vault_key, item_key.as_bytes(), &EncryptionTag::ItemKey),
                    None,
                )],
                "LastToken": null
            }
        });

        let fixture = {
            let group_key = group_key.clone();
            seeded(seeds, move |request: &Request| match request.route() {
                ("GET", "/pass/v1/share") => Some(response(200, shares.to_string())),
                ("GET", "/pass/v1/share/group-share/key") => Some(response(200, keys.to_string())),
                ("GET", "/pass/v1/share/group-share/item") => {
                    Some(response(200, items.to_string()))
                }
                ("GET", "/core/v4/groups/group-value") => Some(response(200, group.to_string())),
                ("GET", "/core/v4/keys/all") => Some(public_keys(&[&group_key])),
                _ => None,
            })
            .await
        };

        let read = fixture.client.shares().await;
        match readable {
            true => {
                let shares = read.unwrap();
                assert_eq!(shares[0].name(), Some("Personal"));
                let items = fixture.client.items(shares[0].id()).await.unwrap();
                assert_eq!(items[0].name(), "Team login");
            }
            false => {
                let error = read.unwrap_err();
                assert!(matches!(error, Error::InvalidShareKeySignature { .. }));
            }
        }
    }
}
