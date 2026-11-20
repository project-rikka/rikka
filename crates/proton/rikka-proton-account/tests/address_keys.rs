use crate::support::fixture::{
    EMAIL, PASSWORD, SALT, Server, address, address_key, new_key, ok, passphrase, token, user_key,
};
use rikka_proton_account::{
    AddressId, AddressKeyFlags, Error, KeyId, SignedKeyList, SignedKeyListCheck,
};
use rikka_proton_pgp::{LockedKey, Pgp, PrivateKey, PublicKey};
use rikka_test_proton::{lock_key, response};
use serde_json::{Value, json};

const OLD_PASSWORD: &str = "the password from before";
const OLD_SALT: &str = "AAECAwQFBgcICQoLDA0ODw==";

struct Fixture {
    server: Server,
    user_key: PrivateKey,
    address_key: PrivateKey,
}

fn generate_fixture(pgp: &Pgp) -> Fixture {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let original = new_key(pgp, EMAIL);
    let token = token(pgp, &user);

    Fixture {
        server: Server::new(
            vec![user_key(
                "user-key",
                &lock_key(&user, passphrase(PASSWORD, SALT)),
                true,
            )],
            vec![address(
                "address-value",
                EMAIL,
                vec![address_key(
                    "address-key",
                    &lock_key(&original, &token.secret),
                    Some(&token),
                    true,
                )],
            )],
        ),
        user_key: user,
        address_key: original,
    }
}

fn sent_list(sent: &Value) -> SignedKeyList {
    serde_json::from_value(sent["SignedKeyList"].clone()).unwrap()
}

fn public(armored: &Value) -> PublicKey {
    LockedKey::import(armored.as_str().unwrap_or_default())
        .unwrap()
        .public_key()
}

#[tokio::test(flavor = "multi_thread")]
async fn new_primary_address_key_listed_first() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("POST", "/core/v4/keys/address") => {
                let sent = request.json();
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "Key": {
                            "ID": "new-key",
                            "PrivateKey": sent["PrivateKey"],
                            "Token": sent["Token"],
                            "Signature": sent["Signature"],
                            "Primary": 1,
                            "Flags": 3
                        }
                    })
                    .to_string(),
                ))
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let created = account
        .create_address_key(&keys, &AddressId::from("address-value"))
        .primary(true)
        .execute()
        .await
        .unwrap();

    assert_eq!(created.id().as_str(), "new-key");
    assert!(created.is_migrated());

    let sent = original
        .server
        .last_body("POST", "/core/v4/keys/address")
        .unwrap();
    assert_eq!(sent["AddressID"], "address-value");
    assert_eq!(sent["Primary"], 1);

    let new_key = public(&sent["PrivateKey"]);
    let list = sent_list(&sent);
    assert_eq!(
        list.check(&pgp, &[original.address_key.public_key(), new_key.clone()]),
        SignedKeyListCheck::Verified
    );
    let items = list.key_list().unwrap().into_items();
    assert!(items[0].matches_key(&new_key) && items[0].is_primary());
    assert!(items[1].matches_key(&original.address_key.public_key()) && !items[1].is_primary());
}

#[tokio::test(flavor = "multi_thread")]
async fn key_changes_carry_resulting_key_list() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let second = new_key(&pgp, EMAIL);
    let second_token = token(&pgp, &original.user_key);
    let mut second_key = address_key(
        "second-key",
        &lock_key(&second, &second_token.secret),
        Some(&second_token),
        true,
    );
    second_key["Primary"] = json!(0);
    original.server.with(|state| {
        state.addresses[0]["Keys"]
            .as_array_mut()
            .unwrap()
            .push(second_key);
    });
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/keys/address-key/flags")
            | ("PUT", "/core/v4/keys/second-key/primary")
            | ("POST", "/core/v4/keys/address/second-key/delete") => Some(ok()),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();
    let first = KeyId::from("address-key");
    let second_id = KeyId::from("second-key");

    account
        .set_address_key_flags(&keys, &first, AddressKeyFlags::NOT_COMPROMISED)
        .await
        .unwrap();
    account
        .set_primary_address_key(&keys, &second_id)
        .await
        .unwrap();
    let refused = account.delete_address_key(&keys, &first).await.unwrap_err();
    assert!(matches!(refused, Error::PrimaryKeyDeletion(ref key) if key == &first));
    account.delete_address_key(&keys, &second_id).await.unwrap();

    let flagged = original
        .server
        .last_body("PUT", "/core/v4/keys/address-key/flags")
        .unwrap();
    assert_eq!(flagged["Flags"], 1);
    let items = sent_list(&flagged).key_list().unwrap().into_items();
    assert_eq!(items[0].flags(), AddressKeyFlags::NOT_COMPROMISED);

    let promoted = original
        .server
        .last_body("PUT", "/core/v4/keys/second-key/primary")
        .unwrap();
    assert_eq!(promoted["Primary"], 1);
    let items = sent_list(&promoted).key_list().unwrap().into_items();
    assert!(items[0].matches_key(&second.public_key()) && items[0].is_primary());
    assert!(!items[1].is_primary());

    let deleted = original
        .server
        .last_body("POST", "/core/v4/keys/address/second-key/delete")
        .unwrap();
    let items = sent_list(&deleted).key_list().unwrap().into_items();
    assert_eq!(items.len(), 1);
    assert!(items[0].matches_key(&original.address_key.public_key()));
}

struct Inactive {
    server: Server,
    active_address: PrivateKey,
    revived_address: PrivateKey,
}

fn with_inactive_user_key(pgp: &Pgp) -> Inactive {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let old_user = new_key(pgp, "not_for_email_use@domain.tld");
    let active_address = new_key(pgp, EMAIL);
    let revived_address = new_key(pgp, EMAIL);
    let active_token = token(pgp, &user);
    let old_token = token(pgp, &old_user);

    let mut old_key = user_key(
        "old-key",
        &lock_key(&old_user, passphrase(OLD_PASSWORD, OLD_SALT)),
        false,
    );
    old_key["Primary"] = json!(0);
    let mut old_address_key = address_key(
        "old-address-key",
        &lock_key(&revived_address, &old_token.secret),
        Some(&old_token),
        false,
    );
    old_address_key["Primary"] = json!(0);

    let server = Server::new(
        vec![
            user_key(
                "user-key",
                &lock_key(&user, passphrase(PASSWORD, SALT)),
                true,
            ),
            old_key,
        ],
        vec![address(
            "address-value",
            EMAIL,
            vec![
                address_key(
                    "address-key",
                    &lock_key(&active_address, &active_token.secret),
                    Some(&active_token),
                    true,
                ),
                old_address_key,
            ],
        )],
    );
    server.with(|state| {
        state
            .salts
            .push(json!({"ID": "old-key", "KeySalt": OLD_SALT}));
    });

    Inactive {
        server,
        active_address,
        revived_address,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn reactivated_user_key_restores_address_keys() {
    let pgp = Pgp::new();
    let inactive = with_inactive_user_key(&pgp);
    let account = inactive
        .server
        .account(|server, request| match request.route() {
            ("PUT", "/core/v4/keys/user/old-key") => {
                let sent = request.json();
                server.with(|state| {
                    state.user_keys[1]["PrivateKey"] = sent["PrivateKey"].clone();
                    state.user_keys[1]["Active"] = json!(1);
                    state.addresses[0]["Keys"][1]["Active"] = json!(1);
                });
                Some(ok())
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();
    assert_eq!(keys.unlocked_key_count(), 2);

    let reactivated = account
        .reactivate_key(&keys, &KeyId::from("old-key"))
        .old_password(OLD_PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(reactivated.unlocked_key_count(), 4);

    let sent = inactive
        .server
        .last_body("PUT", "/core/v4/keys/user/old-key")
        .unwrap();
    LockedKey::import(sent["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(passphrase(PASSWORD, SALT))
        .unwrap();
    assert_eq!(
        sent["AddressKeyFingerprints"],
        json!([inactive.revived_address.fingerprint().as_str()])
    );

    let list: SignedKeyList =
        serde_json::from_value(sent["SignedKeyLists"]["address-value"].clone()).unwrap();
    assert_eq!(
        list.check(
            &pgp,
            &[
                inactive.active_address.public_key(),
                inactive.revived_address.public_key()
            ]
        ),
        SignedKeyListCheck::Verified
    );
    let reactivated = list
        .item_for(&inactive.revived_address.public_key())
        .unwrap();
    assert!(!reactivated.is_primary());
    assert!(!reactivated.flags().can_encrypt());
    assert!(reactivated.flags().can_verify());
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_old_password_reactivates_nothing() {
    let pgp = Pgp::new();
    let inactive = with_inactive_user_key(&pgp);
    let account = inactive.server.account(|_, _| None).await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let error = account
        .reactivate_key(&keys, &KeyId::from("old-key"))
        .old_password("not the old password")
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(error, Error::KeyNotReactivated(ref key) if key.as_str() == "old-key"));
    assert!(
        inactive
            .server
            .last_body("PUT", "/core/v4/keys/user/old-key")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn organization_activated_key_relocked_for_member() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "not_for_email_use@domain.tld");
    let original = new_key(&pgp, EMAIL);
    let activation_secret = "an activation token from the organisation";
    let activation = pgp
        .encrypt()
        .to(user.public_key())
        .signed_by(user.clone())
        .armored_message(activation_secret)
        .unwrap();
    let mut key = address_key(
        "address-key",
        &lock_key(&original, activation_secret),
        None,
        true,
    );
    key["Activation"] = Value::String(activation);

    let server = Server::new(
        vec![user_key(
            "user-key",
            &lock_key(&user, passphrase(PASSWORD, SALT)),
            true,
        )],
        vec![address("address-value", EMAIL, vec![key])],
    );
    let account = server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/keys/address-key/activate") => Some(ok()),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();
    assert!(keys.address_key(&KeyId::from("address-key")).is_some());

    let activated = account.activate_address_keys(&keys).await.unwrap();

    assert_eq!(activated, 1);
    let sent = server
        .last_body("PUT", "/core/v4/keys/address-key/activate")
        .unwrap();
    LockedKey::import(sent["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(passphrase(PASSWORD, SALT))
        .unwrap();
    assert_eq!(
        sent_list(&sent).check(&pgp, &[original.public_key()]),
        SignedKeyListCheck::Verified
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn encryption_and_type_changes_rewrite_flags() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/addresses/address-value/encryption")
            | ("PUT", "/core/v4/addresses/address-value/type") => Some(ok()),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();
    let id = AddressId::from("address-value");

    account
        .address_encryption(&keys, &id)
        .encrypt(false)
        .execute()
        .await
        .unwrap();
    account
        .convert_to_internal_address(&keys, &id)
        .await
        .unwrap();

    let encryption = original
        .server
        .last_body("PUT", "/core/v4/addresses/address-value/encryption")
        .unwrap();
    assert_eq!(encryption["Encrypt"], 0);
    assert_eq!(encryption["Sign"], 1);
    let flags = sent_list(&encryption).key_list().unwrap().into_items()[0].flags();
    assert!(!flags.encrypts_email());
    assert!(flags.expects_signed_email());

    let converted = original
        .server
        .last_body("PUT", "/core/v4/addresses/address-value/type")
        .unwrap();
    assert_eq!(converted["Type"], 3);
    let flags = sent_list(&converted).key_list().unwrap().into_items()[0].flags();
    assert!(flags.encrypts_email() && flags.expects_signed_email());
    assert_eq!(
        sent_list(&converted).check(&pgp, &[original.address_key.public_key()]),
        SignedKeyListCheck::Verified
    );
}
