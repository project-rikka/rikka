use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::IsIdentity as _;
use rikka_proton_key_transparency::{Proof, ProofKind, Realm, VrfKey};
use rikka_proton_pgp::{sha256, sha512};
use serde_json::json;

pub(crate) struct Prover {
    secret: Scalar,
    public: [u8; 32],
}

impl Prover {
    pub(crate) fn new(seed: u8) -> Prover {
        let secret = Scalar::from_bytes_mod_order([seed; 32]);
        let public = (ED25519_BASEPOINT_POINT * secret).compress().to_bytes();
        Prover { secret, public }
    }

    pub(crate) fn realm(&self) -> Realm {
        Realm::new("keytransparency.test", VrfKey::new(self.public))
    }

    pub(crate) fn prove(&self, alpha: &[u8]) -> (String, [u8; 64]) {
        let h = hash_to_curve(&self.public, alpha);
        let gamma = h * self.secret;
        let mut nonce_input = self.secret.to_bytes().to_vec();
        nonce_input.extend_from_slice(alpha);
        let k = Scalar::from_bytes_mod_order_wide(&sha512(&nonce_input));
        let challenge = hash_points(&[h, gamma, ED25519_BASEPOINT_POINT * k, h * k]);
        let mut wide = [0u8; 32];
        wide[..16].copy_from_slice(&challenge[..16]);
        let c = Scalar::from_bytes_mod_order(wide);
        let s = k + c * self.secret;

        let mut proof = gamma.compress().to_bytes().to_vec();
        proof.extend_from_slice(&challenge[..16]);
        proof.extend_from_slice(&s.to_bytes());

        let mut output_input = vec![0x03, 0x03];
        output_input.extend_from_slice(gamma.mul_by_cofactor().compress().as_bytes());
        output_input.push(0x00);
        (rikka_types::hex::encode(proof), sha512(&output_input))
    }
}

fn hash_to_curve(public: &[u8; 32], alpha: &[u8]) -> EdwardsPoint {
    (0..=u8::MAX)
        .find_map(|counter| {
            let mut input = vec![0x03, 0x01];
            input.extend_from_slice(public);
            input.extend_from_slice(alpha);
            input.extend_from_slice(&[counter, 0x00]);
            let digest = sha512(&input);
            let compressed = CompressedEdwardsY::from_slice(&digest[..32]).ok()?;
            let point = compressed.decompress()?;
            if point.compress() != compressed {
                return None;
            }
            let point = point.mul_by_cofactor();
            (!point.is_identity()).then_some(point)
        })
        .unwrap()
}

fn hash_points(points: &[EdwardsPoint]) -> [u8; 64] {
    let mut input = vec![0x03, 0x02];
    for point in points {
        input.extend_from_slice(point.compress().as_bytes());
    }
    input.push(0x00);
    sha512(&input)
}

pub(crate) fn path(output: &[u8; 64], revision: u32) -> [u8; 32] {
    let mut path = [0u8; 32];
    path[..28].copy_from_slice(&output[..28]);
    path[28..].copy_from_slice(&revision.to_be_bytes());
    path
}

pub(crate) fn leaf(content: &str, min_epoch_id: u32) -> [u8; 32] {
    let mut input = sha256(content.as_bytes()).to_vec();
    input.extend_from_slice(&min_epoch_id.to_be_bytes());
    sha256(&input)
}

pub(crate) struct Tree {
    pub(crate) neighbors: Vec<Option<String>>,
    pub(crate) root: String,
}

pub(crate) fn tree(
    path: &[u8; 32],
    leaf: [u8; 32],
    from_first_neighbor: bool,
    filled: impl Fn(usize) -> bool,
) -> Tree {
    let neighbors: Vec<Option<String>> = (0..256)
        .map(|depth| {
            filled(depth)
                .then(|| rikka_types::hex::encode(sha256([u8::try_from(depth).unwrap_or(0), 7])))
        })
        .collect();

    let mut node = leaf;
    let mut started = !from_first_neighbor;
    for (depth, neighbor) in neighbors.iter().enumerate().rev() {
        if !started && neighbor.is_none() {
            continue;
        }
        started = true;
        let sibling = neighbor.as_ref().map_or([0u8; 32], |encoded| {
            let mut sibling = [0u8; 32];
            rikka_types::hex::decode_to_slice(encoded, &mut sibling).unwrap();
            sibling
        });
        let bit = (path[depth / 8] >> (7 - depth % 8)) & 1;
        let mut input = Vec::with_capacity(64);
        if bit == 1 {
            input.extend_from_slice(&sibling);
            input.extend_from_slice(&node);
        } else {
            input.extend_from_slice(&node);
            input.extend_from_slice(&sibling);
        }
        node = sha256(&input);
    }
    Tree {
        neighbors,
        root: rikka_types::hex::encode(node),
    }
}

pub(crate) fn proof(kind: ProofKind, neighbors: &[Option<String>], verifier: &str) -> Proof {
    serde_json::from_value(json!({
        "Type": i32::from(kind),
        "Neighbors": neighbors,
        "Verifier": verifier,
        "ObsolescenceToken": null
    }))
    .unwrap()
}
