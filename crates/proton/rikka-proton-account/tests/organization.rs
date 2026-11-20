use std::sync::{Arc, Mutex};

use crate::support::fixture::{
    EMAIL, PASSWORD, SALT, Server, Token, address, address_key, migrated, new_key, ok, passphrase,
    token, user_key,
};
use rikka_proton_account::{AuthDevice, Error, Unprivatization};
use rikka_proton_pgp::{
    LockedKey, Pgp, PrivateKey, SignatureContext, SymmetricKey, VerificationContext, unarmor,
};
use rikka_test_proton::{Request, ResponseFuture, lock_key, response};
use serde_json::{Value, json};

const ADMIN: &str = "admin@proton.test";
const SECRET: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

struct Invited {
    organization: PrivateKey,
    unprivatization: Value,
}

fn invited(pgp: &Pgp, fingerprint_signer: &PrivateKey) -> Invited {
    let organization = new_key(pgp, "not_for_email_use@domain.tld");
    let fingerprint = organization.public_key().sha256_fingerprints()[0].clone();
    let fingerprint_signature = pgp
        .sign()
        .with_key(fingerprint_signer.clone())
        .with_context(SignatureContext::new("account.organization-fingerprint").critical(true))
        .as_utf8()
        .armored_detached(fingerprint.as_str())
        .unwrap();
    let invitation = json!({"Address": EMAIL, "Revision": 1}).to_string();
    let invitation_signature = pgp
        .sign()
        .with_key(organization.clone())
        .with_context(
            SignatureContext::new("account.unprivatization-invitation-data").critical(true),
        )
        .as_utf8()
        .armored_detached(&invitation)
        .unwrap();

    Invited {
        unprivatization: json!({
            "Code": 1000,
            "State": 1,
            "InvitationData": invitation,
            "InvitationSignature": invitation_signature,
            "InvitationEmail": EMAIL,
            "AdminEmail": ADMIN,
            "OrgKeyFingerprintSignature": fingerprint_signature,
            "OrgPublicKey": organization.public_key().export_armored().unwrap(),
            "PrivateIntent": false
        }),
        organization,
    }
}

fn admin_keys(admin: &PrivateKey) -> ResponseFuture {
    response(
        200,
        json!({
            "Code": 1000,
            "Address": {"Keys": [{
                "Flags": 3,
                "PublicKey": admin.public_key().export_armored().unwrap()
            }]}
        })
        .to_string(),
    )
}

fn routed(
    invited: Value,
    admin: PrivateKey,
) -> impl Fn(&Server, &Request) -> Option<ResponseFuture> + Send + Sync + 'static {
    move |server, request| match request.route() {
        ("GET", "/core/v4/members/me/unprivatize") => Some(response(200, invited.to_string())),
        ("GET", "/core/v4/keys/all") => Some(admin_keys(&admin)),
        ("POST", "/core/v4/keys/setup") => {
            let sent = request.json();
            let key = &sent["AddressKeys"][0];
            server.with(|state| {
                state.user_keys = vec![user_key(
                    "user-key",
                    sent["PrimaryKey"].as_str().unwrap_or_default(),
                    true,
                )];
                state.salts = vec![json!({"ID": "user-key", "KeySalt": sent["KeySalt"]})];
                state.addresses[0]["Keys"] = json!([{
                    "ID": "address-key",
                    "Flags": 3,
                    "PrivateKey": key["PrivateKey"],
                    "Token": key["Token"],
                    "Signature": key["Signature"],
                    "Primary": 1,
                    "Active": 1
                }]);
            });
            Some(ok())
        }
        ("POST", "/core/v4/members/me/unprivatize") => Some(ok()),
        _ => None,
    }
}

fn decrypt_token(
    pgp: &Pgp,
    organization: &PrivateKey,
    sealed: &Value,
    signer: &PrivateKey,
) -> String {
    let decrypted = pgp
        .decrypt()
        .with_key(organization.clone())
        .verified_by(signer.public_key())
        .with_context(VerificationContext::new(
            "account.key-token.user-unprivatization",
        ))
        .message(sealed.as_str().unwrap_or_default())
        .unwrap();
    assert!(
        decrypted.signature().is_valid(),
        "{}",
        decrypted.signature()
    );

    String::from_utf8(decrypted.into_data()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn new_member_keys_open_for_organization() {
    let pgp = Pgp::new();
    let admin = new_key(&pgp, ADMIN);
    let invited = invited(&pgp, &admin);
    let server = Server::new(
        Vec::new(),
        vec![address("address-value", EMAIL, Vec::new())],
    );
    let account = server
        .account(routed(invited.unprivatization.clone(), admin.clone()))
        .await;

    let unprivatization: Unprivatization = account.unprivatization().await.unwrap();
    let verified = account
        .verify_unprivatization(&unprivatization)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified.invitation_address(), Some(EMAIL));

    let keys = account
        .setup_keys()
        .password(PASSWORD)
        .unprivatization(&verified)
        .execute()
        .await
        .unwrap();

    let sent = server.last_body("POST", "/core/v4/keys/setup").unwrap();
    let address_key = keys
        .address_keys(&"address-value".into())
        .and_then(|ring| ring.private_keys().first().cloned())
        .unwrap();
    let token = decrypt_token(
        &pgp,
        &invited.organization,
        &sent["OrgActivationToken"],
        &address_key,
    );
    LockedKey::import(sent["OrgPrimaryUserKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(token.as_bytes())
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unsigned_organization_key_invitation() {
    let pgp = Pgp::new();
    let admin = new_key(&pgp, ADMIN);
    let stranger = new_key(&pgp, "stranger@proton.test");
    let invited = invited(&pgp, &stranger);
    let server = Server::new(
        Vec::new(),
        vec![address("address-value", EMAIL, Vec::new())],
    );
    let account = server
        .account(routed(invited.unprivatization.clone(), admin.clone()))
        .await;

    let unprivatization = account.unprivatization().await.unwrap();
    let error = account
        .verify_unprivatization(&unprivatization)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::UntrustedOrganization(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn existing_member_unprivatizes_user_keys() {
    let pgp = Pgp::new();
    let admin = new_key(&pgp, ADMIN);
    let invited = invited(&pgp, &admin);
    let original = migrated(&pgp);
    let account = original
        .server
        .account(routed(invited.unprivatization.clone(), admin.clone()))
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let unprivatization = account.unprivatization().await.unwrap();
    let verified = account
        .verify_unprivatization(&unprivatization)
        .await
        .unwrap()
        .unwrap();
    account
        .accept_unprivatization(&keys, &verified)
        .await
        .unwrap();

    let sent = original
        .server
        .last_body("POST", "/core/v4/members/me/unprivatize")
        .unwrap();
    let token = decrypt_token(
        &pgp,
        &invited.organization,
        &sent["OrgActivationToken"],
        &original.address_key,
    );
    let copy = LockedKey::import(sent["OrgUserKeys"][0].as_str().unwrap_or_default())
        .unwrap()
        .unlock(token.as_bytes())
        .unwrap();
    assert_eq!(copy.fingerprint(), original.user_key.fingerprint());
}

struct Organization {
    key: PrivateKey,
    armored: String,
    token: Token,
    signature: String,
}

fn passwordless_organization(pgp: &Pgp, user_key: &PrivateKey) -> Organization {
    let key = new_key(pgp, "not_for_email_use@domain.tld");
    let token = token(pgp, user_key);
    let signature = pgp
        .sign()
        .with_key(user_key.clone())
        .with_context(SignatureContext::new("account.key-token.organization").critical(true))
        .armored_detached(&token.secret)
        .unwrap();

    Organization {
        armored: lock_key(&key, &token.secret),
        key,
        token,
        signature,
    }
}

fn organization_keys(organization: &Organization) -> String {
    json!({
        "Code": 1000,
        "PublicKey": "unused",
        "PrivateKey": organization.armored,
        "Token": organization.token.sealed,
        "Signature": organization.signature,
        "Passwordless": true
    })
    .to_string()
}

fn session_key_of(pgp: &Pgp, key: &PrivateKey, message: &[u8]) -> Vec<u8> {
    pgp.decrypt()
        .with_key(key.clone())
        .session_key(message)
        .unwrap()
        .as_bytes()
        .to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_approves_device_with_organization_key() {
    for manual in [false, true] {
        let pgp = Pgp::new();
        let original = migrated(&pgp);
        let organization = passwordless_organization(&pgp, &original.user_key);

        let member_user = new_key(&pgp, "not_for_email_use@domain.tld");
        let member_address = new_key(&pgp, "member@proton.test");
        let member_secret = "a member key token";
        let member_address_token = token(&pgp, &member_user);
        let activation = pgp
            .encrypt()
            .to(member_address.public_key())
            .as_utf8()
            .armored_message(SECRET)
            .unwrap();

        let member = match manual {
            false => json!({
                "ID": "member-id",
                "Keys": [{
                    "ID": "member-user-key",
                    "PrivateKey": lock_key(&member_user, member_secret),
                    "Token": pgp
                        .encrypt()
                        .to(organization.key.public_key())
                        .signed_by(organization.key.clone())
                        .armored_message(member_secret)
                        .unwrap(),
                    "Primary": 1
                }]
            }),
            true => json!({
                "ID": "member-id",
                "Keys": [],
                "Unprivatization": {
                    "State": 2,
                    "PrivateIntent": false,
                    "InvitationData": null,
                    "InvitationSignature": null,
                    "ActivationToken": pgp
                        .encrypt()
                        .to(organization.key.public_key())
                        .as_utf8()
                        .armored_message(member_secret)
                        .unwrap(),
                    "PrivateKeys": [lock_key(&member_user, member_secret)]
                }
            }),
        };
        let member = Arc::new(Mutex::new(member));
        let member_address_value = address(
            "member-address",
            "member@proton.test",
            vec![address_key(
                "member-address-key",
                &lock_key(&member_address, &member_address_token.secret),
                Some(&member_address_token),
                true,
            )],
        );
        let organization_keys_body = organization_keys(&organization);
        let account = original
            .server
            .account({
                let member = member.clone();
                move |_, request| match request.route() {
                    ("GET", "/core/v4/organizations/keys") => {
                        Some(response(200, organization_keys_body.clone()))
                    }
                    ("GET", "/core/v4/members/member-id") => Some(response(
                        200,
                        json!({"Code": 1000, "Member": *member.lock().unwrap()}).to_string(),
                    )),
                    ("GET", "/core/v4/members/member-id/addresses") => Some(response(
                        200,
                        json!({"Code": 1000, "Total": 1, "Addresses": [member_address_value]})
                            .to_string(),
                    )),
                    ("POST", "/core/v4/members/member-id/keys/unprivatize") => {
                        let sent = request.json();
                        let mut original = member.lock().unwrap();
                        original["Keys"] = json!([{
                            "ID": "member-user-key",
                            "PrivateKey": sent["UserKeys"][0]["OrgPrivateKey"],
                            "Token": sent["UserKeys"][0]["OrgToken"],
                            "Primary": 1
                        }]);
                        original["Unprivatization"] = Value::Null;
                        Some(ok())
                    }
                    ("POST", "/core/v4/members/member-id/devices/reset") => Some(ok()),
                    _ => None,
                }
            })
            .await;
        let keys = account.unlock(PASSWORD).await.unwrap();
        let device: AuthDevice = serde_json::from_value(json!({
            "ID": "member-device",
            "MemberID": "member-id",
            "State": 3,
            "Name": "Laptop",
            "ActivationToken": activation,
            "ActivationAddressID": "member-address",
            "ConfirmationCodeVersion": 1
        }))
        .unwrap();

        account
            .approve_member_device_as_admin(&keys, &device, "6MRK")
            .await
            .unwrap();

        let unprivatized = original
            .server
            .last_body("POST", "/core/v4/members/member-id/keys/unprivatize");
        assert_eq!(unprivatized.is_some(), manual, "manual: {manual}");
        if let Some(unprivatized) = unprivatized {
            let decrypted = pgp
                .decrypt()
                .with_key(organization.key.clone())
                .verified_by(organization.key.public_key())
                .message(
                    unprivatized["UserKeys"][0]["OrgToken"]
                        .as_str()
                        .unwrap_or_default(),
                )
                .unwrap();
            assert!(decrypted.signature().is_valid());
            let address_key = &unprivatized["AddressKeys"][0];
            assert_eq!(address_key["AddressKeyID"], "member-address-key");
            let packet = rikka_proton_api::base64::decode(
                address_key["OrgTokenKeyPacket"]
                    .as_str()
                    .unwrap_or_default(),
            )
            .unwrap();
            assert_eq!(
                session_key_of(&pgp, &organization.key, &packet),
                session_key_of(
                    &pgp,
                    &member_user,
                    &unarmor(&member_address_token.sealed).unwrap()
                )
            );
            assert!(
                pgp.verify()
                    .with_key(organization.key.public_key())
                    .detached(
                        member_address_token.secret.as_bytes(),
                        address_key["OrgSignature"].as_str().unwrap_or_default()
                    )
                    .is_valid()
            );
        }

        let sent = original
            .server
            .last_body("POST", "/core/v4/members/member-id/devices/reset")
            .unwrap();
        assert_eq!(sent["AuthDeviceID"], "member-device");
        assert_eq!(sent["UserKeys"][0]["ID"], "member-user-key");

        let key =
            SymmetricKey::from_bytes(rikka_proton_api::base64::decode(SECRET).unwrap()).unwrap();
        let member_passphrase = key
            .open(
                rikka_proton_api::base64::decode(
                    sent["EncryptedSecret"].as_str().unwrap_or_default(),
                )
                .unwrap(),
                b"account.device-secret",
            )
            .unwrap();
        let relocked = LockedKey::import(
            sent["UserKeys"][0]["PrivateKey"]
                .as_str()
                .unwrap_or_default(),
        )
        .unwrap()
        .unlock(&member_passphrase)
        .unwrap();
        assert_eq!(relocked.fingerprint(), member_user.fingerprint());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_user_key_reseals_organization_token() {
    let pgp = Pgp::new();
    let original = migrated(&pgp);
    let organization = passwordless_organization(&pgp, &original.user_key);
    original.server.with(|state| state.role = 2);
    let organization_keys_body = organization_keys(&organization);
    let account = original
        .server
        .account(move |server, request| match request.route() {
            ("GET", "/core/v4/organizations/keys") => {
                Some(response(200, organization_keys_body.clone()))
            }
            ("POST", "/core/v4/keys/user") => {
                let sent = request.json();
                server.with(|state| {
                    state.user_keys[0]["Primary"] = json!(0);
                    state.user_keys.push(user_key(
                        "new-user-key",
                        sent["PrivateKey"].as_str().unwrap_or_default(),
                        true,
                    ));
                });
                Some(ok())
            }
            ("PUT", "/core/v4/keys/tokens") | ("PUT", "/core/v4/organizations/keys/activate") => {
                Some(ok())
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    account.create_user_key(&keys).execute().await.unwrap();

    let created = original
        .server
        .last_body("POST", "/core/v4/keys/user")
        .unwrap();
    let new_key = LockedKey::import(created["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(passphrase(PASSWORD, SALT))
        .unwrap();
    let activated = original
        .server
        .last_body("PUT", "/core/v4/organizations/keys/activate")
        .unwrap();
    let packet =
        rikka_proton_api::base64::decode(activated["TokenKeyPacket"].as_str().unwrap_or_default())
            .unwrap();
    assert_eq!(
        session_key_of(&pgp, &new_key, &packet),
        session_key_of(
            &pgp,
            &original.user_key,
            &unarmor(&organization.token.sealed).unwrap()
        )
    );
    assert!(
        pgp.verify()
            .with_key(new_key.public_key())
            .with_context(VerificationContext::new("account.key-token.organization"))
            .detached(
                organization.token.secret.as_bytes(),
                activated["Signature"].as_str().unwrap_or_default()
            )
            .is_valid()
    );
}
