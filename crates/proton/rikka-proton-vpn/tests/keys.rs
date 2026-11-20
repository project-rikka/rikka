use rikka_proton_vpn::{KeyPair, X25519PublicKey, X25519SecretKey};

const ALICE_SECRET: &str = "dwdtCnMYpX08FsFyUbJmRd9ML4frwJkqsXf7pR25LCo=";
const ALICE_PUBLIC: &str = "hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=";

const SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC_DER: &str = "MCowBQYDK2VwAyEA11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
const PRIVATE_DER: &str = "MC4CAQAwBQYDK2VwBCIEIJ1hsZ3v/VpguoRK9JLsLMREScVpezJpGXA7rAMcrn9g";
const X25519_SECRET: &str = "MHyDhk8oM8tCei7xwAoBPP3/J2jZgMCjpSDwBpBN6U8=";

#[test]
fn x25519_rfc_vector_and_base64() {
    let secret = X25519SecretKey::from_base64(ALICE_SECRET).unwrap();
    assert_eq!(secret.public_key().to_base64(), ALICE_PUBLIC);

    let public = X25519PublicKey::from_base64(ALICE_PUBLIC).unwrap();
    assert_eq!(
        public.fingerprint(),
        "bRtYIAIm5YN0OIqo7TkehSfX76EBXtSnyMbB3fYUaOvSysLNUsy7EEvlXGBgQDJbq7S97f/Jyt9PU/8RJQwKIQ=="
    );
    let encoded = serde_json::to_string(&public).unwrap();
    assert_eq!(encoded, format!("\"{ALICE_PUBLIC}\""));
    assert_eq!(
        serde_json::from_str::<X25519PublicKey>(&encoded).unwrap(),
        public
    );
    assert!(serde_json::from_str::<X25519PublicKey>("\"AAAA\"").is_err());
}

#[test]
fn key_pair_encodes_der_and_pem() {
    let pair = KeyPair::from_seed(SEED);

    assert_eq!(pair.public_key_der_base64(), PUBLIC_DER);
    assert_eq!(pair.private_key_der_base64(), PRIVATE_DER);
    assert_eq!(
        pair.public_key_pem(),
        format!("-----BEGIN PUBLIC KEY-----\n{PUBLIC_DER}\n-----END PUBLIC KEY-----\n")
    );
    assert_eq!(
        pair.private_key_pem(),
        format!("-----BEGIN PRIVATE KEY-----\n{PRIVATE_DER}\n-----END PRIVATE KEY-----\n")
    );
    assert_eq!(pair.x25519_secret_key().to_base64(), X25519_SECRET);
    assert_eq!(
        pair.x25519_public_key(),
        pair.x25519_secret_key().public_key()
    );
}

#[test]
fn debug_output_redacts_secret_keys() {
    let secret = X25519SecretKey::from_base64(ALICE_SECRET).unwrap();
    assert!(!format!("{secret:?}").contains("dwdtCnMY"));

    let pair = KeyPair::generate();
    assert!(!format!("{pair:?}").contains(&pair.seed_base64()));
}
