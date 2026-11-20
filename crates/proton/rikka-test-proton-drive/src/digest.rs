#[must_use]
pub fn lookup_hash(hash_key: &[u8], name: &str) -> String {
    rikka_types::hex::encode(rikka_proton_pgp::hmac(
        rikka_proton_pgp::Hash::Sha256,
        hash_key,
        name,
    ))
}

#[must_use]
pub fn base64_digest(data: &[u8]) -> String {
    rikka_proton_api::base64::encode(rikka_proton_pgp::sha256(data))
}

#[must_use]
pub fn sha1_hex(data: &[u8]) -> String {
    let mut digest = rikka_proton_pgp::Sha1::new();
    digest.update(data);
    rikka_types::hex::encode(digest.finalize())
}

pub(crate) fn manifest(blocks: &[Vec<u8>]) -> Vec<u8> {
    blocks
        .iter()
        .flat_map(|block| rikka_proton_pgp::sha256(block).to_vec())
        .collect()
}
