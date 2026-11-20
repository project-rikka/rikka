use rikka_proton_key_transparency::{CertificateIssuer, Epoch, EpochCertificate, Failure, Realm};
use rikka_test_certificate_transparency::chrome_log_list;

use crate::fixtures::{EPOCH_1509, LETS_ENCRYPT_CHAIN, ZERO_SSL_CHAIN};

const DAY: u64 = 24 * 60 * 60;
const YEAR: u64 = 365 * DAY;
const LETS_ENCRYPT_CERTIFIED: u64 = 1_785_143_442;
const ZERO_SSL_CERTIFIED: u64 = 1_786_453_876;

fn parsed(pem: &str) -> EpochCertificate {
    EpochCertificate::parse(pem).unwrap()
}

#[test]
fn epoch_certificate_chains_to_pinned_roots() {
    let lets_encrypt = parsed(LETS_ENCRYPT_CHAIN);
    let log_list = chrome_log_list();
    assert_eq!(
        lets_encrypt.verify(
            CertificateIssuer::LetsEncrypt,
            &log_list,
            LETS_ENCRYPT_CERTIFIED + DAY
        ),
        Ok(())
    );
    assert_eq!(lets_encrypt.not_before(), 1_785_139_957);
    assert_eq!(
        lets_encrypt.alternative_name().as_deref(),
        Some(
            "99f32f15d517a8e398058a089156d3eb.d25e6470e55717dbcf0d8429fad71e41.1785143442.6500.1.keytransparency.ch"
        )
    );

    let zero_ssl = parsed(ZERO_SSL_CHAIN);
    assert_eq!(
        zero_ssl.verify(
            CertificateIssuer::ZeroSsl,
            &log_list,
            ZERO_SSL_CERTIFIED + DAY
        ),
        Ok(())
    );
}

#[test]
fn rejects_invalid_epoch_certificates() {
    let lets_encrypt = parsed(LETS_ENCRYPT_CHAIN);
    let log_list = chrome_log_list();
    let refused = [
        (
            CertificateIssuer::LetsEncrypt,
            LETS_ENCRYPT_CERTIFIED + YEAR,
        ),
        (
            CertificateIssuer::LetsEncrypt,
            LETS_ENCRYPT_CERTIFIED - YEAR,
        ),
        (CertificateIssuer::ZeroSsl, LETS_ENCRYPT_CERTIFIED + DAY),
    ];
    for (issuer, at) in refused {
        assert!(matches!(
            lets_encrypt.verify(issuer, &log_list, at),
            Err(Failure::CertificateChain(_))
        ));
    }
    assert_eq!(
        lets_encrypt.verify(
            CertificateIssuer::Other(7),
            &log_list,
            LETS_ENCRYPT_CERTIFIED + DAY
        ),
        Err(Failure::UnknownIssuer(7))
    );

    let leaf_end = LETS_ENCRYPT_CHAIN
        .find("-----END CERTIFICATE-----")
        .unwrap()
        + "-----END CERTIFICATE-----".len();
    let intermediates_start = ZERO_SSL_CHAIN.find("-----END CERTIFICATE-----").unwrap()
        + "-----END CERTIFICATE-----".len();
    let broken = format!(
        "{}{}",
        &LETS_ENCRYPT_CHAIN[..leaf_end],
        &ZERO_SSL_CHAIN[intermediates_start..]
    );
    assert!(matches!(
        parsed(&broken).verify(
            CertificateIssuer::LetsEncrypt,
            &log_list,
            LETS_ENCRYPT_CERTIFIED + DAY
        ),
        Err(Failure::CertificateChain(_))
    ));

    assert!(matches!(
        EpochCertificate::parse("corrupt"),
        Err(Failure::MalformedCertificate(_))
    ));
}

#[test]
fn epoch_hash_bound_to_certificate() {
    let epoch: Epoch = serde_json::from_str(EPOCH_1509).unwrap();
    let claimed = epoch.claimed_time();
    let log_list = chrome_log_list();

    assert_eq!(
        epoch.verify(&Realm::production(), &log_list, claimed),
        Err(Failure::InsufficientTimestamps)
    );
    assert!(matches!(
        epoch.verify(&Realm::atlas(), &log_list, claimed),
        Err(Failure::AlternativeNameMismatch { .. })
    ));

    let mut tampered: serde_json::Value = serde_json::from_str(EPOCH_1509).unwrap();
    tampered["TreeHash"] =
        serde_json::Value::from("0b505c668eb5f8459b1684ee47b40e0983b5e79c86b5e345ff49ca386c785c4d");
    let tampered: Epoch = serde_json::from_value(tampered).unwrap();
    assert_eq!(
        tampered.verify(&Realm::production(), &log_list, claimed),
        Err(Failure::ChainHashMismatch)
    );
}
