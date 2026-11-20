use rikka_proton_account::{Address, AddressKey, AddressStatus, AddressType, User, UserKey};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn round_trip<T: Serialize + DeserializeOwned>(wire: Value) -> T {
    let parsed: T = serde_json::from_value(wire.clone()).unwrap();
    let serialized = serde_json::to_value(&parsed).unwrap();

    for (name, value) in wire.as_object().unwrap() {
        if name == "Code" || value.is_null() {
            continue;
        }
        assert_eq!(
            serialized.get(name).map(without_nulls),
            Some(without_nulls(value)),
            "field {name} was dropped"
        );
    }

    parsed
}

fn without_nulls(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(name, value)| (name.clone(), without_nulls(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_nulls).collect()),
        other => other.clone(),
    }
}

#[test]
fn user_response_keeps_every_field() {
    let user: User = round_trip(json!({
        "ID": "user-value",
        "Name": "tester",
        "DisplayName": "Tester",
        "Email": "tester@proton.test",
        "UsedSpace": 1234,
        "MaxSpace": 5678,
        "MaxUpload": 91,
        "CreateTime": 1_700_000_000,
        "Credit": 0,
        "Currency": "CHF",
        "Type": 1,
        "Role": 2,
        "Private": 1,
        "Subscribed": 1,
        "Services": 5,
        "Delinquent": 0,
        "Keys": [{
            "ID": "user-key",
            "Version": 3,
            "PrivateKey": "armored",
            "Fingerprint": "fingerprint",
            "Activation": null,
            "Primary": 1,
            "Active": 1,
            "RecoverySecret": null,
            "RecoverySecretSignature": null
        }]
    }));

    assert_eq!(user.id().as_str(), "user-value");
    assert_eq!(user.display_name(), Some("Tester"));
    assert!(user.is_private());
    assert_eq!(user.used_space(), 1234);
    assert_eq!(user.role(), 2);

    let key: &UserKey = user.primary_key().unwrap();
    assert_eq!(key.id().as_str(), "user-key");
    assert!(key.is_primary());
    assert!(key.is_active());
    assert_eq!(key.private_key(), "armored");
}

#[test]
fn address_response_keeps_every_field() {
    let address: Address = round_trip(json!({
        "ID": "address-value",
        "DomainID": "domain-value",
        "Email": "tester@proton.test",
        "Send": 1,
        "Receive": 1,
        "Status": 1,
        "Type": 5,
        "Order": 3,
        "DisplayName": "Tester",
        "Signature": "signature",
        "HasKeys": 1,
        "Keys": [{
            "ID": "address-key",
            "Version": 3,
            "Flags": 3,
            "PrivateKey": "armored",
            "Token": "token",
            "Signature": "signature",
            "Fingerprint": "fingerprint",
            "Fingerprints": ["fingerprint"],
            "Activation": null,
            "Primary": 1,
            "Active": 1
        }]
    }));

    assert_eq!(address.status(), AddressStatus::Enabled);
    assert_eq!(address.kind(), AddressType::External);
    assert_eq!(address.order(), 3);
    assert!(address.can_send());
    assert!(address.has_keys());

    let key: &AddressKey = address.primary_key().unwrap();
    assert_eq!(key.id().as_str(), "address-key");
    assert_eq!(key.token(), Some("token"));
    assert_eq!(key.flags(), 3);
    assert!(key.is_migrated());
}
