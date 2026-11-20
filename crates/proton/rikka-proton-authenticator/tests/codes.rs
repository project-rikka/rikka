use rikka_proton_authenticator::{Algorithm, Base32, Error, Hotp, Steam, Totp};

const SHA1_SEED: &[u8] = b"12345678901234567890";
const SHA256_SEED: &[u8] = b"12345678901234567890123456789012";
const SHA512_SEED: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

fn timestamp(seconds: i64) -> jiff::Timestamp {
    jiff::Timestamp::from_second(seconds).unwrap()
}

#[test]
fn rfc_6238_vectors() {
    let vectors = [
        (59, "94287082", "46119246", "90693936"),
        (1_111_111_109, "07081804", "68084774", "25091201"),
        (1_111_111_111, "14050471", "67062674", "99943326"),
        (1_234_567_890, "89005924", "91819424", "93441116"),
        (2_000_000_000, "69279037", "90698825", "38618901"),
        (20_000_000_000, "65353130", "77737706", "47863826"),
    ];
    for (seconds, sha1, sha256, sha512) in vectors {
        for (seed, algorithm, expected) in [
            (SHA1_SEED, Algorithm::Sha1, sha1),
            (SHA256_SEED, Algorithm::Sha256, sha256),
            (SHA512_SEED, Algorithm::Sha512, sha512),
        ] {
            let totp = Totp {
                algorithm,
                digits: 8,
                ..Totp::new(Base32::encode(seed))
            };
            assert_eq!(
                totp.code_at(timestamp(seconds)).unwrap(),
                expected,
                "{algorithm:?} at {seconds}"
            );
        }
    }
}

#[test]
fn rfc_4226_vectors() {
    let expected = [
        "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871",
        "520489",
    ];
    let hotp = Hotp::new(Base32::encode(SHA1_SEED));
    for (counter, code) in expected.into_iter().enumerate() {
        assert_eq!(
            hotp.code_for(counter as u64).unwrap(),
            code,
            "counter {counter}"
        );
    }
}

#[test]
fn steam_code_vectors() {
    assert_eq!(
        Steam::new("STEAMKEY")
            .code_at(timestamp(1_742_298_317))
            .unwrap(),
        "VKFDN"
    );

    let base64 = Steam::new("////EjRWeJq83vARIjNEVWZ3iJk=");
    let base32 = Steam::new("77776ERUKZ4JVPG66AISEM2EKVTHPCEZ");
    let moment = timestamp(1_700_000_000);
    assert_eq!(base64.code_at(moment).unwrap(), "64YGY");
    assert_eq!(base32.code_at(moment).unwrap(), "64YGY");
}

#[test]
fn base32_follows_rfc_4648() {
    let vectors = [
        ("", ""),
        ("f", "MY======"),
        ("fo", "MZXQ===="),
        ("foo", "MZXW6==="),
        ("foob", "MZXW6YQ="),
        ("fooba", "MZXW6YTB"),
        ("foobar", "MZXW6YTBOI======"),
    ];
    for (plain, encoded) in vectors {
        assert_eq!(Base32::encode_padded(plain.as_bytes()), encoded);
        assert_eq!(
            Base32::encode(plain.as_bytes()),
            encoded.trim_end_matches('=')
        );
        for input in [
            encoded.to_owned(),
            encoded.trim_end_matches('=').to_owned(),
            encoded.to_ascii_lowercase(),
        ] {
            assert_eq!(Base32::decode(&input).unwrap(), plain.as_bytes());
        }
    }
    for text in ["1", "MZXW6!", "MZ XW6", "MZXW60"] {
        assert!(matches!(
            Base32::decode(text).unwrap_err(),
            Error::MalformedBase32
        ));
    }
}
