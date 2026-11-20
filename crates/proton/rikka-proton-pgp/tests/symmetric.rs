use rikka_proton_pgp::{Hash, NonceLength, Sha1, SymmetricKey, hmac, sha256};

#[test]
fn symmetric_key_requires_matching_aad() {
    let key = SymmetricKey::generate();
    let sealed = key.seal(b"the content", b"vaultcontent").unwrap();
    let again = key.seal(b"the content", b"vaultcontent").unwrap();
    let nonce = NonceLength::Standard.bytes();

    assert_ne!(sealed[..nonce], again[..nonce]);
    assert_eq!(key.open(&sealed, b"vaultcontent").unwrap(), b"the content");
    assert!(key.open(&sealed, b"itemcontent").is_err());
    assert!(key.open(&sealed[..nonce], b"vaultcontent").is_err());
    assert!(
        SymmetricKey::generate()
            .open(&sealed, b"vaultcontent")
            .is_err()
    );

    let long = key.clone().with_nonce(NonceLength::Long);
    let sealed = long.seal(b"payload", b"").unwrap();
    assert_eq!(sealed.len(), 16 + 7 + 16);
    assert_eq!(long.open(&sealed, b"").unwrap(), b"payload");
    assert!(key.open(&sealed, b"").is_err());

    assert!(SymmetricKey::from_bytes([7u8; 31]).is_err());
}

#[test]
fn digest_vectors() {
    let mut streamed = Sha1::new();
    streamed.update(b"a");
    streamed.update(b"bc");
    assert_eq!(streamed.finalize()[..4], [0xa9, 0x99, 0x3e, 0x36]);
    assert_eq!(sha256(b"abc")[..4], [0xba, 0x78, 0x16, 0xbf]);
    assert_eq!(
        hmac(
            Hash::Sha256,
            b"key",
            b"The quick brown fox jumps over the lazy dog"
        )[..4],
        [0xf7, 0xbc, 0x83, 0xf4],
    );
}
