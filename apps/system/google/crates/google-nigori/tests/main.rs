use google_nigori::{Derivation, Nigori};
use rikka_types::hex;

const PLAIN: &[u8] = b"test, test, 1, 2, 3";
const MAC_OVER_CIPHERTEXT: &str = "NNYlnzaaLPXWXyzz8J+u4OKgLiKRBPu2GJdjHWk0m3ADZrJhnmer30\
    Zgiy4Ulxlfh6fmS71k8rop+UvSJdL1k/fcNLJ1C6sY5Z86ijyl1Jo=";
const MAC_OVER_IV_AND_CIPHERTEXT: &str = "WaACU5CT+wnttSXFQmXgVjv4W1AqKdYURasywlvTZwv2RYvrOs3nc507\
    759fvB1tMi32oOU77PSw1qg3ySF8kdF9VYEknEX+eC8vJni9vaE=";

fn password() -> Nigori {
    Nigori::derive(&Derivation::Pbkdf2, b"password")
}

#[test]
fn key_name_matches_reference() {
    assert_eq!(
        password().name(),
        "ibGL7ymU0Si+eYCXGS6SBHPFT+JCYiB6GDOYqj6vIwEiWJ7RENSHxmIQ8Q3rXd/UnZUmFHYB+jSIbthQADXvrQ=="
    );
}

#[test]
fn derivations_match_reference_keys() {
    let cases = [
        (
            Derivation::Pbkdf2,
            "4596bf346572497d92b2a0e2146d93c1",
            "2292ad9db96fe590b22a58db50f6f545",
        ),
        (
            Derivation::Scrypt {
                salt: b"alpensalz".to_vec(),
            },
            "8aa735e0091339a5e51da3b3dd1b328a",
            "a7e73611968dfd2bca5b3382aed451ba",
        ),
    ];
    for (derivation, encryption_key, mac_key) in cases {
        let nigori = Nigori::derive(&derivation, b"hunter2");
        assert_eq!(hex::encode(nigori.encryption_key()), encryption_key);
        assert_eq!(hex::encode(nigori.mac_key()), mac_key);
    }
}

#[test]
fn reference_ciphertexts_decrypt() {
    for golden in [MAC_OVER_CIPHERTEXT, MAC_OVER_IV_AND_CIPHERTEXT] {
        let plain = password().decrypt(golden).expect("a golden blob decrypts");
        assert_eq!(plain.as_slice(), PLAIN);
    }
}

#[test]
fn encryption_round_trips_with_fresh_iv() {
    let nigori = password();
    let (first, second) = (nigori.encrypt(PLAIN), nigori.encrypt(PLAIN));

    assert_ne!(first, second);
    for blob in [first, second, nigori.encrypt(b"")] {
        assert!(nigori.decrypt(&blob).is_some());
    }
    let imported = Nigori::import(nigori.encryption_key(), nigori.mac_key()).expect("a key");
    assert_eq!(imported.name(), nigori.name());
}

#[test]
fn tampered_or_foreign_blobs_are_refused() {
    let nigori = password();
    let mut tampered = nigori.encrypt(PLAIN).into_bytes();
    tampered[30] = if tampered[30] == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(tampered).expect("base64 text");

    assert!(nigori.decrypt(&tampered).is_none());
    assert!(nigori.decrypt("not base64").is_none());
    let other = Nigori::derive(&Derivation::Pbkdf2, b"other");
    assert!(other.decrypt(MAC_OVER_CIPHERTEXT).is_none());
    assert!(Nigori::import(&[0; 15], &[0; 16]).is_none());
}
