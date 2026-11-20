use rikka_proton_api::wire_enum;
use std::fmt;

use rikka_proton_account::SignedKeyList;
use serde::{Deserialize, Serialize};

use crate::failure::Failure;
use crate::identifier::Identifier;
use crate::merkle::{self, Hash, Hashing};
use crate::realm::Realm;
use crate::vrf::VrfOutput;

wire_enum! {
    pub enum ProofKind {
        Absence = 0,
        Existence = 1,
        Obsolescence = 2,
    }
}

impl fmt::Display for ProofKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofKind::Absence => formatter.write_str("absence"),
            ProofKind::Existence => formatter.write_str("existence"),
            ProofKind::Obsolescence => formatter.write_str("obsolescence"),
            ProofKind::Other(other) => write!(formatter, "unknown kind {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proof {
    #[serde(rename = "Type")]
    kind: ProofKind,
    #[serde(rename = "Neighbors")]
    neighbors: Vec<Option<String>>,
    #[serde(rename = "Verifier")]
    verifier: String,
    #[serde(
        rename = "ObsolescenceToken",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    obsolescence_token: Option<String>,
}

impl Proof {
    #[must_use]
    pub fn kind(&self) -> ProofKind {
        self.kind
    }

    #[must_use]
    pub fn neighbors(&self) -> &[Option<String>] {
        &self.neighbors
    }

    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    pub fn verify_absence(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
        revision: u32,
    ) -> Result<(), Failure> {
        self.expect_kind(ProofKind::Absence)?;
        let output = self.vrf_output(realm, identifier)?;
        self.verify_root(
            tree_hash,
            &merkle::path(&output, revision),
            merkle::empty_leaf(),
            Hashing::FromFirstNeighbor,
        )
    }

    pub fn verify_absence_of_all(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
    ) -> Result<(), Failure> {
        self.verify_absence(realm, tree_hash, identifier, 0)?;
        if !merkle::revision_subtree_is_empty(&self.neighbors) {
            return Err(Failure::RevisionSubtreeNotEmpty);
        }
        Ok(())
    }

    pub fn verify_existence(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
        signed_key_list: &SignedKeyList,
    ) -> Result<(), Failure> {
        self.expect_kind(ProofKind::Existence)?;
        let data = signed_key_list
            .data()
            .ok_or(Failure::IncompleteSignedKeyList)?;
        self.verify_leaf(realm, tree_hash, identifier, signed_key_list, data)
    }

    pub fn verify_obsolescence(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
        signed_key_list: &SignedKeyList,
    ) -> Result<(), Failure> {
        self.expect_kind(ProofKind::Obsolescence)?;
        let token = signed_key_list
            .obsolescence_token()
            .ok_or(Failure::IncompleteSignedKeyList)?;
        if !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Failure::MalformedObsolescenceToken);
        }
        self.verify_leaf(realm, tree_hash, identifier, signed_key_list, token)
    }

    pub fn verify_inclusion(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
        signed_key_list: &SignedKeyList,
    ) -> Result<(), Failure> {
        match self.kind {
            ProofKind::Obsolescence => {
                self.verify_obsolescence(realm, tree_hash, identifier, signed_key_list)
            }
            _ => self.verify_existence(realm, tree_hash, identifier, signed_key_list),
        }
    }

    fn verify_leaf(
        &self,
        realm: &Realm,
        tree_hash: &str,
        identifier: &Identifier,
        signed_key_list: &SignedKeyList,
        content: &str,
    ) -> Result<(), Failure> {
        let min_epoch_id = signed_key_list
            .min_epoch_id()
            .ok_or(Failure::IncompleteSignedKeyList)?;
        let revision = match signed_key_list.revision() {
            0 => return Err(Failure::IncompleteSignedKeyList),
            revision => revision,
        };
        let output = self.vrf_output(realm, identifier)?;
        self.verify_root(
            tree_hash,
            &merkle::path(&output, revision),
            merkle::leaf(content, min_epoch_id),
            Hashing::FromLeaf,
        )
    }

    fn expect_kind(&self, expected: ProofKind) -> Result<(), Failure> {
        if self.kind == expected {
            Ok(())
        } else {
            Err(Failure::UnexpectedProof {
                expected,
                found: self.kind,
            })
        }
    }

    fn vrf_output(&self, realm: &Realm, identifier: &Identifier) -> Result<VrfOutput, Failure> {
        let proof = rikka_types::hex::decode(&self.verifier).map_err(|_| Failure::VrfProof)?;
        realm
            .vrf_key()
            .verify(identifier.as_str().as_bytes(), &proof)
    }

    fn verify_root(
        &self,
        tree_hash: &str,
        path: &Hash,
        leaf: Hash,
        hashing: Hashing,
    ) -> Result<(), Failure> {
        let expected = merkle::decode_hash(tree_hash).ok_or(Failure::MalformedEpoch)?;
        let root = merkle::root(&self.neighbors, path, leaf, hashing)?;
        if root == expected {
            Ok(())
        } else {
            Err(Failure::TreeHashMismatch)
        }
    }
}
