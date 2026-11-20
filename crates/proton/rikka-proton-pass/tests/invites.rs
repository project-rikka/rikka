use std::sync::{Arc, Mutex};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_pass::{
    EncryptionKey, EncryptionTag, Error, Invitation, ItemId, MemberRole, NewUserInviteState,
    ShareId, Vault,
};
use rikka_proton_pgp::{
    KeyRing, Pgp, PrivateKey, SignatureContext, SignatureStatus, VerificationContext,
};
use serde_json::{Value, json};

use crate::support::{
    EMAIL, ITEM, SHARE, fixture, new_key, public_keys, seal_base64, share_json, share_keys_json,
};
use rikka_test_proton::{Request, response, success};

const FRIEND: &str = "friend@proton.test";
const INVITER: &str = "inviter@proton.test";

fn decrypt_invite_key(encrypted: &Value, invitee: &PrivateKey, inviter: &PrivateKey) -> Vec<u8> {
    let message = BASE64_STANDARD.decode(encrypted.as_str().unwrap()).unwrap();
    let decrypted = Pgp::new()
        .decrypt()
        .with_key(invitee.clone())
        .verified_by(inviter.public_key())
        .with_context(VerificationContext::new("pass.invite.vault.existing-user"))
        .message(message)
        .unwrap();

    assert!(
        decrypted.signature().is_valid(),
        "{:?}",
        decrypted.signature()
    );
    decrypted.into_data()
}

fn encrypt_invite_key(
    key: &EncryptionKey,
    invitee: &PrivateKey,
    inviter: &PrivateKey,
    context: &str,
) -> String {
    let message = Pgp::new()
        .encrypt()
        .to(invitee.public_key())
        .signed_by(inviter.clone())
        .with_context(SignatureContext::new(context).critical(true))
        .message(key.as_bytes())
        .unwrap();

    BASE64_STANDARD.encode(message)
}

fn new_user_signature(signer: &PrivateKey, email: &str, key: &EncryptionKey) -> String {
    let mut body = email.as_bytes().to_vec();
    body.push(b'|');
    body.extend_from_slice(key.as_bytes());

    let signature = Pgp::new()
        .sign()
        .with_key(signer.clone())
        .with_context(SignatureContext::new("pass.invite.vault.new-user").critical(true))
        .detached(body)
        .unwrap();

    BASE64_STANDARD.encode(signature)
}

#[tokio::test(flavor = "multi_thread")]
async fn existing_user_invite_seals_keys() {
    let invitee = new_key(FRIEND);
    let invitee_keys = invitee.clone();
    let fixture = fixture(move |request: &Request| match request.route() {
        ("GET", "/core/v4/keys/all") => Some(public_keys(&[&invitee_keys])),
        ("POST", path) if path == format!("/pass/v1/share/{SHARE}/invite") => Some(success()),
        _ => None,
    })
    .await;

    let cases = [
        (
            Invitation::new(FRIEND, MemberRole::Editor),
            1,
            Value::Null,
            fixture.share_key.clone(),
        ),
        (
            Invitation::new(FRIEND, MemberRole::Viewer).for_item(ItemId::from(ITEM)),
            2,
            json!(ITEM),
            fixture.item_key.clone(),
        ),
    ];

    for (invitation, target, item, expected) in cases {
        let role = invitation.role().id();
        let invited = fixture
            .client
            .invite(&fixture.share(), invitation)
            .await
            .unwrap();
        assert!(!invited.is_new_user());

        let request = fixture
            .record
            .last("POST", &format!("/pass/v1/share/{SHARE}/invite"))
            .json();
        assert_eq!(request["Email"], FRIEND);
        assert_eq!(request["TargetType"], target);
        assert_eq!(request["ShareRoleID"], role);
        assert_eq!(request["ItemID"], item);
        assert_eq!(request["Keys"][0]["KeyRotation"], 1);
        assert_eq!(
            decrypt_invite_key(&request["Keys"][0]["Key"], &invitee, &fixture.address),
            expected.as_bytes()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn new_user_invite_is_signed() {
    let fixture = fixture(|request: &Request| match request.route() {
        ("GET", "/core/v4/keys/all") => Some(response(
            422,
            r#"{"Code":33102,"Error":"This address does not exist"}"#,
        )),
        ("POST", path) if path == format!("/pass/v1/share/{SHARE}/invite/new_user/batch") => {
            Some(success())
        }
        _ => None,
    })
    .await;

    let invited = fixture
        .client
        .invite_all(
            &fixture.share(),
            [Invitation::new(FRIEND, MemberRole::Manager)
                .with_expiration(jiff::Timestamp::from_second(1_800_000_000).unwrap())],
        )
        .await
        .unwrap();
    assert!(invited[0].is_new_user());

    let request = fixture
        .record
        .last(
            "POST",
            &format!("/pass/v1/share/{SHARE}/invite/new_user/batch"),
        )
        .json();
    let invite = &request["NewUserInvites"][0];
    assert_eq!(invite["Email"], FRIEND);
    assert_eq!(invite["ShareRoleID"], "1");
    assert_eq!(invite["ExpirationTime"], 1_800_000_000i64);

    let mut body = FRIEND.as_bytes().to_vec();
    body.push(b'|');
    body.extend_from_slice(fixture.share_key.as_bytes());
    let signature = BASE64_STANDARD
        .decode(invite["Signature"].as_str().unwrap())
        .unwrap();
    let status = Pgp::new()
        .verify()
        .with_key(fixture.address.public_key())
        .with_context(VerificationContext::new("pass.invite.vault.new-user"))
        .detached(body, signature);

    assert!(status.is_valid(), "{status:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn accept_invite_opens_vault() {
    let inviter = new_key(INVITER);
    let vault_key = EncryptionKey::generate();
    let accepted = Arc::new(Mutex::new(None::<Value>));

    let fixture = {
        let (inviter, vault_key, accepted) = (inviter.clone(), vault_key.clone(), accepted.clone());
        let invitee: Arc<Mutex<Option<(PrivateKey, PrivateKey)>>> = Arc::default();
        let invitee_keys = invitee.clone();
        let fixture = fixture(move |request: &Request| match request.route() {
            ("GET", "/core/v4/keys/all") => Some(public_keys(&[&inviter])),
            ("GET", "/pass/v1/invite") => {
                let (address, _) = invitee_keys.lock().unwrap().clone()?;
                let vault = Vault {
                    name: "Shared".to_owned(),
                    description: String::new(),
                    display: None,
                };
                let invite = |token: &str, context: &str| {
                    json!({
                        "InviteToken": token,
                        "TargetType": 1,
                        "TargetID": "vault-of-joined",
                        "InviterEmail": INVITER,
                        "InvitedEmail": EMAIL,
                        "InvitedAddressID": "address-value",
                        "Keys": [{
                            "KeyRotation": 1,
                            "Key": encrypt_invite_key(&vault_key, &address, &inviter, context)
                        }],
                        "VaultData": {
                            "Content": seal_base64(&vault_key, vault.encode(), &EncryptionTag::VaultContent),
                            "ContentKeyRotation": 1,
                            "ContentFormatVersion": 1,
                            "MemberCount": 2,
                            "ItemCount": 7
                        },
                        "FromNewUser": false,
                        "CreateTime": 1_700_000_000i64
                    })
                };
                Some(response(200,
                    json!({
                        "Code": 1000,
                        "Invites": [
                            invite("token-value", "pass.invite.vault.existing-user"),
                            invite("forged-value", "pass.invite.somewhere-else")
                        ]
                    })
                    .to_string(),
                ))
            }
            ("POST", "/pass/v1/invite/token-value") => {
                *accepted.lock().unwrap() = Some(request.json());
                Some(response(200,
                    json!({"Code": 1000, "Share": share_json("joined", &vault_key)}).to_string(),
                ))
            }
            ("GET", "/pass/v1/share/joined/key") => {
                let (_, user) = invitee_keys.lock().unwrap().clone()?;
                let keys = rikka_proton_account::UserKeys::new(
                    KeyRing::new().with_private(user.clone()),
                    user,
                );
                Some(response(200, share_keys_json(&keys, &vault_key).to_string()))
            }
            _ => None,
        })
        .await;
        *invitee.lock().unwrap() = Some((fixture.address.clone(), fixture.user.clone()));
        fixture
    };

    let invites = fixture.client.invites().await.unwrap();
    assert_eq!(invites.len(), 1);
    let invite = &invites[0];
    assert_eq!(
        invite.vault().map(|vault| vault.name.as_str()),
        Some("Shared")
    );
    assert_eq!(invite.item_count(), Some(7));

    let share = fixture.client.accept_invite(invite).await.unwrap();
    assert_eq!(share.id(), &ShareId::from("joined"));
    assert_eq!(share.name(), Some("Personal"));

    let posted = accepted.lock().unwrap().clone().unwrap();
    let resealed = BASE64_STANDARD
        .decode(posted["Keys"][0]["Key"].as_str().unwrap())
        .unwrap();
    let decrypted = EncryptionKey::unwrap(
        &Pgp::new(),
        &fixture.user_keys(),
        &ShareId::from("joined"),
        resealed,
    )
    .unwrap();
    assert_eq!(decrypted.as_bytes(), vault_key.as_bytes());
}

#[tokio::test(flavor = "multi_thread")]
async fn new_user_invite_confirm_requires_signature() {
    let invitee = new_key(FRIEND);
    let foreign_key = new_key(INVITER);
    let signer: Arc<Mutex<Option<(PrivateKey, EncryptionKey)>>> = Arc::default();

    let fixture = {
        let (invitee, foreign_key, signer_keys) =
            (invitee.clone(), foreign_key.clone(), signer.clone());
        fixture(move |request: &Request| match request.route() {
            ("GET", "/core/v4/keys/all") => Some(public_keys(&[&invitee])),
            ("GET", path) if path == format!("/pass/v1/share/{SHARE}/invite") => {
                let (address, key) = signer_keys.lock().unwrap().clone()?;
                let invite = |id: &str, by: &PrivateKey| {
                    json!({
                        "NewUserInviteID": id,
                        "State": 2,
                        "TargetType": 1,
                        "TargetID": "vault-value",
                        "ShareRoleID": "2",
                        "InvitedEmail": FRIEND,
                        "InviterEmail": EMAIL,
                        "Signature": new_user_signature(by, FRIEND, &key),
                        "CreateTime": 1,
                        "ModifyTime": 2
                    })
                };
                Some(response(200,
                    json!({
                        "Code": 1000,
                        "Invites": [],
                        "NewUserInvites": [invite("signed-value", &address), invite("forged-value", &foreign_key)]
                    })
                    .to_string(),
                ))
            }
            ("POST", path) if path.starts_with(&format!("/pass/v1/share/{SHARE}/invite/new_user/")) => {
                Some(success())
            }
            _ => None,
        })
        .await
    };
    *signer.lock().unwrap() = Some((fixture.address.clone(), fixture.share_key.clone()));

    let invites = fixture
        .client
        .share_invites(&fixture.share())
        .await
        .unwrap();
    let pending = invites.new_user_invites();
    assert_eq!(pending[0].state(), NewUserInviteState::Ready);

    fixture
        .client
        .confirm_new_user_invite(&fixture.share(), &pending[0])
        .await
        .unwrap();
    let request = fixture
        .record
        .last(
            "POST",
            &format!("/pass/v1/share/{SHARE}/invite/new_user/signed-value/keys"),
        )
        .json();
    assert_eq!(
        decrypt_invite_key(&request["Keys"][0]["Key"], &invitee, &fixture.address),
        fixture.share_key.as_bytes()
    );

    let error = fixture
        .client
        .confirm_new_user_invite(&fixture.share(), &pending[1])
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidInviteSignature {
            status: SignatureStatus::Invalid { .. } | SignatureStatus::UnknownKey { .. },
            ..
        }
    ));
    assert!(
        fixture
            .record
            .matching(
                "POST",
                &format!("/pass/v1/share/{SHARE}/invite/new_user/forged-value/keys"),
            )
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn member_changes_update_share() {
    let fixture = fixture(|request: &Request| match request.route() {
        ("GET", path) if path == format!("/pass/v1/share/{SHARE}/user") => Some(response(
            200,
            json!({
                "Code": 1000,
                "Total": 1,
                "Shares": [{
                    "ShareID": "member-value",
                    "UserName": "friend",
                    "UserEmail": FRIEND,
                    "Owner": false,
                    "TargetType": 1,
                    "TargetID": "vault-value",
                    "Permission": 0,
                    "ShareRoleID": "3",
                    "IsGroupShare": false,
                    "CreateTime": 1
                }]
            })
            .to_string(),
        )),
        ("PUT" | "DELETE", _) => Some(success()),
        _ => None,
    })
    .await;
    let share = fixture.share();
    let member = ShareId::from("member-value");

    let members = fixture.client.members(&share).await.unwrap();
    assert_eq!(members[0].user_email(), FRIEND);
    assert_eq!(members[0].role(), MemberRole::Viewer.into());

    fixture
        .client
        .set_member_role(&share, &member, MemberRole::Manager, None)
        .await
        .unwrap();
    fixture
        .client
        .transfer_ownership(&share, &member)
        .await
        .unwrap();
    fixture.client.remove_member(&share, &member).await.unwrap();

    let role = fixture
        .record
        .last("PUT", &format!("/pass/v1/share/{SHARE}/user/member-value"))
        .json();
    assert_eq!(role, json!({"ShareRoleID": "1"}));
    let owner = fixture
        .record
        .last("PUT", &format!("/pass/v1/vault/{SHARE}/owner"))
        .json();
    assert_eq!(owner, json!({"NewOwnerShareID": "member-value"}));
    fixture.record.last(
        "DELETE",
        &format!("/pass/v1/share/{SHARE}/user/member-value"),
    );
}
