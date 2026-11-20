use crate::support::fixture::{EMAIL, Server, new_key};
use rikka_proton_account::{
    AddressKeyFlags, Error, PublishedAddress, SignedKeyList, SignedKeyListCheck, SignedKeyListItem,
};
use rikka_proton_pgp::{Pgp, PrivateKey};
use rikka_test_proton::response;
use serde_json::json;

fn signed(pgp: &Pgp, keys: &[&PrivateKey], signer: &PrivateKey) -> SignedKeyList {
    let items: Vec<SignedKeyListItem> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            SignedKeyListItem::new(
                &key.public_key(),
                index == 0,
                AddressKeyFlags::NOT_COMPROMISED | AddressKeyFlags::NOT_OBSOLETE,
            )
        })
        .collect();

    SignedKeyList::sign(pgp, &items, signer).unwrap()
}

fn published(data: &str, signature: Option<&str>, obsolete: bool) -> SignedKeyList {
    let mut list = json!({"Data": data});
    if let Some(signature) = signature {
        list["Signature"] = json!(signature);
    }
    if obsolete {
        list["ObsolescenceToken"] = json!("obsolete-token");
    }

    serde_json::from_value(list).unwrap()
}

#[test]
fn signed_key_list_requires_listed_signer() {
    let pgp = Pgp::new();
    let primary = new_key(&pgp, EMAIL);
    let secondary = new_key(&pgp, EMAIL);
    let stranger = new_key(&pgp, "stranger@proton.test");
    let both = [primary.public_key(), secondary.public_key()];

    let complete = signed(&pgp, &[&primary, &secondary], &primary);
    let partial = signed(&pgp, &[&primary], &primary);
    let forged = signed(&pgp, &[&primary, &secondary], &stranger);
    let unparsable = published("not json", complete.signature(), false);
    let unsigned = published(complete.data().unwrap_or_default(), None, false);
    let obsolete: SignedKeyList =
        serde_json::from_value(json!({"ObsolescenceToken": "obsolete-token"})).unwrap();

    let cases = [
        (&complete, "verified"),
        (&partial, "mismatched"),
        (&forged, "unsigned"),
        (&unparsable, "malformed"),
        (&unsigned, "malformed"),
        (&obsolete, "obsolete"),
        (&SignedKeyList::default(), "absent"),
    ];

    for (list, expected) in cases {
        let check = list.check(&pgp, &both);
        let named = match check {
            SignedKeyListCheck::Verified => "verified",
            SignedKeyListCheck::Mismatched => "mismatched",
            SignedKeyListCheck::Unsigned(_) => "unsigned",
            SignedKeyListCheck::Malformed => "malformed",
            SignedKeyListCheck::Obsolete => "obsolete",
            SignedKeyListCheck::Absent => "absent",
            _ => "unknown",
        };
        assert_eq!(named, expected, "{check}");
    }

    let items = complete.key_list().unwrap().into_items();
    assert!(items[0].is_primary() && !items[1].is_primary());
    assert!(items[1].matches_key(&secondary.public_key()));
    assert_eq!(
        complete
            .item_for(&secondary.public_key())
            .map(|item| item.flags()),
        Some(AddressKeyFlags::from_bits(3))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_forged_signed_key_list() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, EMAIL);
    let stranger = new_key(&pgp, "stranger@proton.test");
    let armored = key.public_key().export_armored().unwrap();
    let forged = signed(&pgp, &[&key], &stranger);

    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(move |_, request| match request.route() {
            ("GET", "/core/v4/keys/all") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "ProtonMX": true,
                    "IsProton": 1,
                    "Address": {
                        "Keys": [{"Flags": 3, "PublicKey": armored}],
                        "SignedKeyList": forged
                    }
                })
                .to_string(),
            )),
            _ => None,
        })
        .await;

    let error = account
        .public_keys("friend@proton.test")
        .send()
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::UntrustedSignedKeyList { ref email, .. } if email == "friend@proton.test"
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn accepts_matching_signed_key_list() {
    let pgp = Pgp::new();
    let key = new_key(&pgp, EMAIL);
    let armored = key.public_key().export_armored().unwrap();
    let honest = signed(&pgp, &[&key], &key);

    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(move |_, request| match request.route() {
            ("GET", "/core/v4/keys/all") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "Address": {
                        "Keys": [{"Flags": 3, "PublicKey": armored}],
                        "SignedKeyList": honest
                    }
                })
                .to_string(),
            )),
            _ => None,
        })
        .await;

    let info = account
        .public_keys("friend@proton.test")
        .send()
        .await
        .unwrap()
        .into_public_keys()
        .unwrap();

    assert_eq!(
        info.address().check_signed_key_list(&pgp),
        SignedKeyListCheck::Verified
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unpublished_address_is_not_failure() {
    let cases = [
        (33102, Some(false)),
        (33103, Some(true)),
        (33101, Some(false)),
        (33104, Some(false)),
        (2001, None),
    ];

    for (code, external) in cases {
        let server = Server::new(Vec::new(), Vec::new());
        let account = server
            .account(move |_, request| match request.route() {
                ("GET", "/core/v4/keys/all") => Some(response(
                    422,
                    json!({"Code": code, "Error": "no keys"}).to_string(),
                )),
                _ => None,
            })
            .await;

        let looked_up = account
            .public_keys("friend@proton.test")
            .internal_only()
            .send()
            .await;

        match (looked_up, external) {
            (Ok(PublishedAddress::Unpublished(refusal)), Some(external)) => {
                assert_eq!(refusal.code.value(), code);
                assert_eq!(
                    PublishedAddress::Unpublished(refusal).is_external_domain(),
                    external
                );
            }
            (Err(Error::Api(_)), None) => {}
            (looked_up, _) => panic!("unexpected lookup result for {code}: {looked_up:?}"),
        }
    }
}
