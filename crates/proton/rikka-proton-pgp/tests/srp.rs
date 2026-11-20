use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_pgp::{Error, HashVersion, Modulus, Srp};
use rikka_test_proton::{FakeSrp, MODULUS, SIGNED_MODULUS};

const VERIFIER_PASSWORD: &str = "123";
const VERIFIER_SALT: &str = "SzHkg+YYA/eN1A==";
const EXPECTED_VERIFIER: &str = "j2o8z9G+Xm5t07Y6D7rauq3bNi6v0ZqnM1nWuZHS8PgtQOl4Xgh8LjuzulhX1izaOqeIoW221Z/LDVkrUZzxAXwFdi5LfxMN+RHPJCg0Uk5OcigQHsO1xTMuk3hvoIXO7yIXXs2oCqpBwKNfuhMNjcwVlgjyh5ZC4FzhSV2lwlP7KE1me/USAOfq4FbW7KtDtvxX8fk6hezWIz9X8/bcAHwQkHobqOVTCE81Lg+WL7s4sMed72YHwx5p6S/YGm558zrZmeETv6PuS4MRkQ8vPRrIvmzPEQDUiOXCaqfLkGvBFeCbBjNtBM8AlbWcW8XE+gcb/GwWH8cHinzd4ddh4A==";

const BCRYPT_PREFIX_LEN: usize = 29;

fn modulus() -> Modulus {
    Modulus::from_signed(SIGNED_MODULUS).unwrap()
}

#[test]
fn signed_modulus_verifies() {
    let modulus = modulus();
    assert_eq!(modulus.as_str(), MODULUS);
    assert_eq!(modulus.id(), None);
    assert_eq!(modulus.clone().with_id("abc").id(), Some("abc"));
}

#[test]
fn rejects_tampered_modulus() {
    let tampered = SIGNED_MODULUS.replace("y6Ttuf", "y6Ttua");
    let error = Modulus::from_signed(&tampered).unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidModulus(_) | Error::Malformed(_)
    ));
}

#[test]
fn password_verifier_vector() {
    let verifier = Srp::new()
        .register()
        .password(VERIFIER_PASSWORD)
        .salt(VERIFIER_SALT)
        .modulus(modulus().with_id("ModulusID"))
        .verifier()
        .unwrap();

    assert_eq!(verifier.version(), HashVersion::V4);
    assert_eq!(verifier.salt(), VERIFIER_SALT);
    assert_eq!(verifier.verifier(), EXPECTED_VERIFIER);
    assert_eq!(verifier.modulus_id(), Some("ModulusID"));
}

#[test]
fn mailbox_passphrase_vectors() {
    let cases = [
        (
            "password",
            "imK9IHsRcA2Zsv+yROZgbw==",
            "$2y$10$gkI7GFqPa.0Xqt8wPMXeZuQ.Gd9rSsqE0xQ8Qcf0Q9ckInb4hIzOu",
        ),
        (
            "a loooong password~~~?\n\n",
            "6hGAnuIZ7Dgf3f/diExc6A==",
            "$2y$10$4fE.lsGX5Bed1d9bgCva4.5x5DgJBFXMNlS93R3YxYVVxR7Jq8V3q",
        ),
        (
            "t",
            "rNBLbNt14Nuo9lzr6/QpVg==",
            "$2y$10$pL/JZLrz2Lsm7jxp49OnTeQSN9nb4NyO/HCMNYgPTztxZHDF9XRVO",
        ),
    ];

    for (password, salt, expected) in cases {
        let salt = BASE64_STANDARD.decode(salt).unwrap();
        let passphrase = Srp::new().mailbox_passphrase(password, &salt).unwrap();
        assert_eq!(
            passphrase.as_bytes(),
            &expected.as_bytes()[BCRYPT_PREFIX_LEN..]
        );
    }
}

#[test]
fn debug_output_redacts_mailbox_passphrase() {
    let salt = BASE64_STANDARD.decode("imK9IHsRcA2Zsv+yROZgbw==").unwrap();
    let passphrase = Srp::new().mailbox_passphrase("password", &salt).unwrap();
    let debug = format!("{passphrase:?}");
    assert!(!debug.contains("Gd9rSsqE"));
}

#[test]
fn srp_proofs_verify_both_ways() {
    let password = "the account password";
    let modulus = modulus();
    let srp = Srp::new();

    let verifier = srp
        .register()
        .password(password)
        .modulus(modulus.clone())
        .verifier()
        .unwrap();

    let server = FakeSrp::registered(verifier.salt(), verifier.verifier());
    let challenge = server.challenge();

    let proofs = srp
        .authenticate()
        .password(password)
        .version(HashVersion::V4)
        .salt(verifier.salt())
        .modulus(modulus)
        .server_ephemeral(challenge)
        .proofs()
        .unwrap();

    let server_proof = server
        .prove(proofs.client_ephemeral(), proofs.client_proof())
        .unwrap();

    proofs.verify_server_proof(&server_proof).unwrap();
    assert!(matches!(
        proofs.verify_server_proof("not the server proof"),
        Err(Error::ServerProof)
    ));
}
