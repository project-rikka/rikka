use rikka_proton_pgp::{Pgp, Secret, SecretBytes, SessionKey};

#[test]
fn debug_output_redacts_secrets() {
    let secret = Secret::from("password-value");
    assert_eq!(secret.expose(), "password-value");
    assert!(!format!("{secret:?}").contains("password-value"));

    let bytes = SecretBytes::new(*b"hunter2");
    assert!(!format!("{bytes:?}").contains("hunter2"));
    assert!(!format!("{bytes:?}").contains("104"));

    let key = SessionKey::from_bytes([171u8; 32]).unwrap();
    let debug = format!("{key:?}");
    assert!(!debug.contains("171") && !debug.contains("ab"), "{debug}");

    let encryption = format!("{:?}", Pgp::new().encrypt().with_password("hunter2"));
    assert!(!encryption.contains("hunter2"));
}
