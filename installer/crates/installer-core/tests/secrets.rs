use installer_core::{Secret, hash_password};

#[test]
fn hashes_use_libxcrypt_format() {
    let password = Secret::from("correct horse battery");
    let hash = hash_password(&password).expect("system RNG should be available");
    assert!(hash.starts_with("$6$rounds=200000$"), "{hash}");
    assert_eq!(hash.split('$').count(), 5, "{hash}");
    assert!(!hash.contains("correct"));
    assert_ne!(
        hash,
        hash_password(&password).expect("system RNG should be available")
    );
}

#[test]
fn secret_never_reallocates() {
    for mut secret in [
        Secret::new(),
        Secret::default(),
        Secret::from("correct horse battery"),
        Secret::from("correct horse battery").clone(),
    ] {
        let before = secret.as_str().as_ptr();
        let mut refused = false;
        for index in 0..600 {
            let ch = if index % 2 == 0 { 'x' } else { 'あ' };
            refused |= !secret.insert(secret.chars(), ch);
            assert_eq!(secret.as_str().as_ptr(), before, "the buffer moved");
        }
        assert!(refused, "a secret has a fixed capacity");
    }
}

#[test]
fn edits_respect_char_boundaries() {
    let mut secret = Secret::new();
    for (index, ch) in "あaい".chars().enumerate() {
        secret.insert(index, ch);
    }
    secret.remove_before(2);
    assert_eq!(secret.as_str(), "あい");
    assert_eq!(secret.chars(), 2);
}
