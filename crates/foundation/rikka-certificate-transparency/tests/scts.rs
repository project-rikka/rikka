use rikka_certificate_transparency::{Standing, ValidSct};
use rikka_test_certificate_transparency::{Signer, chrome_log_list, log_list_with_states};
use serde_json::json;

use crate::support::epoch_certificate;

const EMBEDDED_SCTS: usize = 2;

#[test]
fn chrome_list_admits_epoch_certificate_timestamps() {
    let certificate = epoch_certificate();
    let list = chrome_log_list();
    let scts = list
        .embedded_scts(&certificate.leaf, &certificate.issuer_public_key_info)
        .unwrap();

    assert!(
        scts.iter().any(|sct| sct.standing() == Standing::Current),
        "{scts:?}"
    );
}

#[test]
fn log_states_decide_which_timestamps_count() {
    let certificate = epoch_certificate();
    let signer = Signer::new();
    let since = json!({"timestamp": "2024-01-01T00:00:00Z"});
    let cases = [
        (json!({"usable": since}), Some(Standing::Current)),
        (json!({"qualified": since}), Some(Standing::Current)),
        (
            json!({"readonly": {
                "timestamp": "2024-01-01T00:00:00Z",
                "final_tree_head": {"sha256_root_hash": "", "tree_size": 1}
            }}),
            Some(Standing::Current),
        ),
        (
            json!({"retired": {"timestamp": "2027-01-01T00:00:00Z"}}),
            Some(Standing::Retired),
        ),
        (
            json!({"retired": {"timestamp": "2026-01-01T00:00:00Z"}}),
            None,
        ),
        (json!({"pending": since}), None),
        (json!({"rejected": since}), None),
        (json!({}), None),
    ];

    for (state, expected) in cases {
        let list = signer.signed(&log_list_with_states("2030-01-01T00:00:00Z", &state));
        let standings: Vec<Standing> = list
            .embedded_scts(&certificate.leaf, &certificate.issuer_public_key_info)
            .unwrap()
            .iter()
            .map(ValidSct::standing)
            .collect();
        let expected: Vec<Standing> = expected
            .map(|standing| vec![standing; EMBEDDED_SCTS])
            .unwrap_or_default();
        assert_eq!(standings, expected, "{state}");
    }
}

#[test]
fn rejects_timestamps_signed_over_another_certificate() {
    let certificate = epoch_certificate();
    let list = chrome_log_list();
    let scts = list
        .embedded_scts(&certificate.leaf, &certificate.leaf)
        .unwrap();

    assert!(scts.is_empty(), "{scts:?}");
}
