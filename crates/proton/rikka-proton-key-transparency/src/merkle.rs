use rikka_proton_pgp::sha256;

use crate::failure::Failure;
use crate::vrf::VrfOutput;

pub(crate) const HASH_LENGTH: usize = 32;
const DEPTH: usize = HASH_LENGTH * 8;
const REVISION_SUBTREE: usize = DEPTH - 32;
const PATH_PREFIX: usize = HASH_LENGTH - 4;
const LEFT_NEIGHBOR: u8 = 1;

pub(crate) type Hash = [u8; HASH_LENGTH];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hashing {
    FromLeaf,
    FromFirstNeighbor,
}

pub(crate) fn path(vrf_output: &VrfOutput, revision: u32) -> Hash {
    let mut path = [0u8; HASH_LENGTH];
    path[..PATH_PREFIX].copy_from_slice(&vrf_output[..PATH_PREFIX]);
    path[PATH_PREFIX..].copy_from_slice(&revision.to_be_bytes());
    path
}

pub(crate) fn leaf(content: &str, min_epoch_id: u32) -> Hash {
    let mut input = Vec::with_capacity(HASH_LENGTH + 4);
    input.extend_from_slice(&sha256(content.as_bytes()));
    input.extend_from_slice(&min_epoch_id.to_be_bytes());
    sha256(&input)
}

pub(crate) fn empty_leaf() -> Hash {
    [0u8; HASH_LENGTH]
}

pub(crate) fn root(
    neighbors: &[Option<String>],
    path: &Hash,
    leaf: Hash,
    hashing: Hashing,
) -> Result<Hash, Failure> {
    if neighbors.len() != DEPTH {
        return Err(Failure::NeighborCount {
            found: neighbors.len(),
        });
    }

    let mut node = leaf;
    let mut started = hashing == Hashing::FromLeaf;
    for (depth, neighbor) in neighbors.iter().enumerate().rev() {
        if !started && neighbor.is_none() {
            continue;
        }
        started = true;

        let sibling = match neighbor {
            Some(encoded) => decode_hash(encoded).ok_or(Failure::MalformedNeighbor)?,
            None => empty_leaf(),
        };
        let bit = (path[depth / 8] >> (7 - depth % 8)) & 1;
        let mut input = [0u8; 2 * HASH_LENGTH];
        let (first, second) = if bit == LEFT_NEIGHBOR {
            (&sibling, &node)
        } else {
            (&node, &sibling)
        };
        input[..HASH_LENGTH].copy_from_slice(first);
        input[HASH_LENGTH..].copy_from_slice(second);
        node = sha256(input);
    }
    Ok(node)
}

pub(crate) fn revision_subtree_is_empty(neighbors: &[Option<String>]) -> bool {
    neighbors
        .get(REVISION_SUBTREE..)
        .is_some_and(|subtree| subtree.iter().all(Option::is_none))
}

pub(crate) fn decode_hash(encoded: &str) -> Option<Hash> {
    let mut hash = [0u8; HASH_LENGTH];
    rikka_types::hex::decode_to_slice(encoded, &mut hash).ok()?;
    Some(hash)
}
