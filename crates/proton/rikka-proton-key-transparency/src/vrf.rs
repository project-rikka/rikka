use std::fmt;

use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::IsIdentity as _;
use rikka_proton_pgp::sha512;

use crate::failure::Failure;

const SUITE: u8 = 0x03;
const HASH_TO_CURVE: u8 = 0x01;
const CHALLENGE: u8 = 0x02;
const PROOF_TO_HASH: u8 = 0x03;
const BACK: u8 = 0x00;
const POINT_LENGTH: usize = 32;
const CHALLENGE_LENGTH: usize = 16;
const PROOF_LENGTH: usize = 2 * POINT_LENGTH + CHALLENGE_LENGTH;

pub type VrfOutput = [u8; 64];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VrfKey([u8; POINT_LENGTH]);

impl VrfKey {
    #[must_use]
    pub const fn new(encoded: [u8; POINT_LENGTH]) -> VrfKey {
        VrfKey(encoded)
    }

    pub fn from_hex(encoded: &str) -> Result<VrfKey, Failure> {
        let mut bytes = [0u8; POINT_LENGTH];
        rikka_types::hex::decode_to_slice(encoded, &mut bytes)
            .map_err(|_| Failure::InvalidVrfKey)?;
        Ok(VrfKey(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; POINT_LENGTH] {
        &self.0
    }

    pub fn verify(&self, alpha: &[u8], proof: &[u8]) -> Result<VrfOutput, Failure> {
        let key = point(&self.0).ok_or(Failure::InvalidVrfKey)?;
        if proof.len() != PROOF_LENGTH {
            return Err(Failure::VrfProof);
        }
        let (gamma, rest) = proof.split_at(POINT_LENGTH);
        let (challenge, response) = rest.split_at(CHALLENGE_LENGTH);

        let gamma = point(gamma).ok_or(Failure::VrfProof)?;
        let c = challenge_scalar(challenge);
        let s = response_scalar(response).ok_or(Failure::VrfProof)?;
        let h = hash_to_curve(&self.0, alpha).ok_or(Failure::VrfProof)?;

        let u = EdwardsPoint::vartime_double_scalar_mul_basepoint(&-c, &key, &s);
        let v = h * s - gamma * c;

        if hash_points(&[h, gamma, u, v])[..CHALLENGE_LENGTH] != *challenge {
            return Err(Failure::VrfProof);
        }
        Ok(proof_to_hash(&gamma))
    }
}

impl fmt::Debug for VrfKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("VrfKey")
            .field(&rikka_types::hex::encode(self.0))
            .finish()
    }
}

fn point(encoded: &[u8]) -> Option<EdwardsPoint> {
    let compressed = CompressedEdwardsY::from_slice(encoded).ok()?;
    let point = compressed.decompress()?;
    (point.compress() == compressed).then_some(point)
}

fn challenge_scalar(challenge: &[u8]) -> Scalar {
    let mut wide = [0u8; 32];
    wide[..CHALLENGE_LENGTH].copy_from_slice(challenge);
    Scalar::from_bytes_mod_order(wide)
}

fn response_scalar(response: &[u8]) -> Option<Scalar> {
    let bytes: [u8; 32] = response.try_into().ok()?;
    Option::from(Scalar::from_canonical_bytes(bytes))
}

fn hash_to_curve(key: &[u8; POINT_LENGTH], alpha: &[u8]) -> Option<EdwardsPoint> {
    (0..=u8::MAX).find_map(|counter| {
        let mut input = Vec::with_capacity(POINT_LENGTH + alpha.len() + 4);
        input.extend_from_slice(&[SUITE, HASH_TO_CURVE]);
        input.extend_from_slice(key);
        input.extend_from_slice(alpha);
        input.extend_from_slice(&[counter, BACK]);
        let candidate = point(&sha512(&input)[..POINT_LENGTH])?.mul_by_cofactor();
        (!candidate.is_identity()).then_some(candidate)
    })
}

fn hash_points(points: &[EdwardsPoint]) -> [u8; 64] {
    let mut input = Vec::with_capacity(points.len() * POINT_LENGTH + 3);
    input.extend_from_slice(&[SUITE, CHALLENGE]);
    for point in points {
        input.extend_from_slice(point.compress().as_bytes());
    }
    input.push(BACK);
    sha512(&input)
}

fn proof_to_hash(gamma: &EdwardsPoint) -> VrfOutput {
    let mut input = Vec::with_capacity(POINT_LENGTH + 3);
    input.extend_from_slice(&[SUITE, PROOF_TO_HASH]);
    input.extend_from_slice(gamma.mul_by_cofactor().compress().as_bytes());
    input.push(BACK);
    sha512(&input)
}
