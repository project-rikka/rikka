use rikka_proton_account::SignedKeyList;
use rikka_proton_key_transparency::{Failure, Identifier, ProofKind};
use serde_json::json;

use crate::support::tree::{Prover, leaf, path, proof, tree};

const DATA: &str = r#"[{"Primary":1,"Flags":3,"Fingerprint":"aa","SHA256Fingerprints":["bb"]}]"#;
const TOKEN: &str = "0000000065f0a1b2c3d4e5f60718293a";

fn included(
    revision: u32,
    min_epoch_id: u32,
    data: Option<&str>,
    token: Option<&str>,
) -> SignedKeyList {
    serde_json::from_value(json!({
        "Data": data,
        "Signature": data.map(|_| "signature"),
        "ObsolescenceToken": token,
        "MinEpochID": min_epoch_id,
        "Revision": revision
    }))
    .unwrap()
}

#[test]
fn inclusion_proofs_bind_content_and_revision() {
    let prover = Prover::new(7);
    let realm = prover.realm();
    let identifier = Identifier::address("alice@proton.me").unwrap();
    let (verifier, output) = prover.prove(identifier.as_str().as_bytes());

    for (kind, data, token) in [
        (ProofKind::Existence, Some(DATA), None),
        (ProofKind::Obsolescence, None, Some(TOKEN)),
    ] {
        let content = data.or(token).unwrap();
        let built = tree(&path(&output, 3), leaf(content, 42), false, |depth| {
            depth % 3 == 0
        });
        let served = proof(kind, &built.neighbors, &verifier);
        let list = included(3, 42, data, token);

        assert_eq!(
            served.verify_inclusion(&realm, &built.root, &identifier, &list),
            Ok(())
        );

        let failures = [
            (included(4, 42, data, token), Failure::TreeHashMismatch),
            (included(3, 43, data, token), Failure::TreeHashMismatch),
            (
                included(3, 0, data, token),
                Failure::IncompleteSignedKeyList,
            ),
        ];
        for (list, failure) in failures {
            assert_eq!(
                served.verify_inclusion(&realm, &built.root, &identifier, &list),
                Err(failure)
            );
        }

        let other = Identifier::address("bob@proton.me").unwrap();
        assert_eq!(
            served.verify_inclusion(&realm, &built.root, &other, &list),
            Err(Failure::VrfProof)
        );
    }
}

#[test]
fn rejects_malformed_proofs() {
    let prover = Prover::new(9);
    let realm = prover.realm();
    let identifier = Identifier::address("alice@proton.me").unwrap();
    let (verifier, output) = prover.prove(identifier.as_str().as_bytes());
    let built = tree(&path(&output, 1), leaf(DATA, 5), false, |depth| depth < 10);
    let list = included(1, 5, Some(DATA), None);

    let absence = proof(ProofKind::Absence, &built.neighbors, &verifier);
    assert_eq!(
        absence.verify_existence(&realm, &built.root, &identifier, &list),
        Err(Failure::UnexpectedProof {
            expected: ProofKind::Existence,
            found: ProofKind::Absence
        })
    );

    let short = proof(ProofKind::Existence, &built.neighbors[1..], &verifier);
    assert_eq!(
        short.verify_existence(&realm, &built.root, &identifier, &list),
        Err(Failure::NeighborCount { found: 255 })
    );

    let uppercase = included(1, 5, None, Some("ABCDEF"));
    let obsolete = proof(ProofKind::Obsolescence, &built.neighbors, &verifier);
    assert_eq!(
        obsolete.verify_obsolescence(&realm, &built.root, &identifier, &uppercase),
        Err(Failure::MalformedObsolescenceToken)
    );
}

#[test]
fn absence_proofs_hash_from_neighbour() {
    let prover = Prover::new(11);
    let realm = prover.realm();
    let identifier = Identifier::catch_all("someone@example.org").unwrap();
    let (verifier, output) = prover.prove(identifier.as_str().as_bytes());

    let built = tree(&path(&output, 0), [0u8; 32], true, |depth| depth < 20);
    let served = proof(ProofKind::Absence, &built.neighbors, &verifier);
    assert_eq!(
        served.verify_absence_of_all(&realm, &built.root, &identifier),
        Ok(())
    );
    assert_eq!(
        served.verify_absence(&realm, &built.root, &identifier, 6),
        Ok(())
    );

    let revisions = tree(&path(&output, 0), [0u8; 32], true, |depth| {
        depth < 20 || depth == 255
    });
    let served = proof(ProofKind::Absence, &revisions.neighbors, &verifier);
    assert_eq!(
        served.verify_absence(&realm, &revisions.root, &identifier, 0),
        Ok(())
    );
    assert_eq!(
        served.verify_absence(&realm, &revisions.root, &identifier, 1),
        Err(Failure::TreeHashMismatch)
    );
    assert_eq!(
        served.verify_absence_of_all(&realm, &revisions.root, &identifier),
        Err(Failure::RevisionSubtreeNotEmpty)
    );
}
