use rikka_proton_account::{
    AddressKeyFlags, Error, KeyDifference, KeyList, KeyListMismatch, SignedKeyListItem,
};
use rikka_proton_pgp::PublicKey;

use crate::fixtures::{V4_KEY, V6_KEY};

const SIGNED_DATA: &str = r#"[{"Primary":1,"Flags":3,"Fingerprint":"4b4f9b2835d7c629e1a544efc1a438bcc21845ce","SHA256Fingerprints":["dfb6288abc061d287c362df18be5cffa4f90d92a4ec06a30ea44668747c3e63b","82d0f72cccede7187dd98ed96d36ea021d4944a030ebf594ee9e03201a8c3183"]},{"Primary":1,"Flags":3,"Fingerprint":"1bf5cc883b677f359c3bb0f30c60633eacc85a8dcce7e47f31cd4c317a3a0613","SHA256Fingerprints":["1bf5cc883b677f359c3bb0f30c60633eacc85a8dcce7e47f31cd4c317a3a0613","762d96c87a3d8719ed83a9fc814d6ee31ec3459b6ebc3c7a349ad47b5bb26500"]}]"#;

fn key(armored: &str) -> PublicKey {
    PublicKey::import(armored).unwrap()
}

fn item(key: &PublicKey, flags: i32, primary: bool) -> SignedKeyListItem {
    SignedKeyListItem::new(key, primary, AddressKeyFlags::from_bits(flags))
}

#[test]
fn published_keys_match_signed_key_list() {
    let signed = KeyList::parse(SIGNED_DATA).unwrap();
    let v4 = key(V4_KEY);
    let v6 = key(V6_KEY);

    assert_eq!(
        signed.compare(&[item(&v6, 3, true), item(&v4, 3, true)]),
        Ok(())
    );
    assert_eq!(
        KeyList::new([item(&v4, 3, true), item(&v6, 3, true)]).to_data(),
        SIGNED_DATA
    );

    let fingerprint = v4.fingerprint().into_string();
    let mismatches = [
        (
            vec![item(&v4, 1, true), item(&v6, 3, true)],
            KeyListMismatch::Key {
                fingerprint: fingerprint.clone(),
                difference: KeyDifference::Flags,
            },
        ),
        (
            vec![item(&v4, 3, false), item(&v6, 3, true)],
            KeyListMismatch::Key {
                fingerprint,
                difference: KeyDifference::Primary,
            },
        ),
        (
            vec![item(&v4, 3, true)],
            KeyListMismatch::Count {
                listed: 1,
                signed: 2,
            },
        ),
        (Vec::new(), KeyListMismatch::NoKeys),
    ];
    for (listed, mismatch) in mismatches {
        assert_eq!(signed.compare(&listed), Err(mismatch));
    }

    assert!(matches!(
        KeyList::parse(r#"[{"Primary":2,"Flags":3,"Fingerprint":"a","SHA256Fingerprints":[]}]"#),
        Err(Error::MalformedKeyList)
    ));
}
