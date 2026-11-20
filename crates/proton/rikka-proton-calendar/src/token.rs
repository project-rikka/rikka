use rikka_proton_pgp::Sha1;

#[must_use]
pub fn attendee_token(canonical_email: &str, uid: &str) -> String {
    let joined = format!("{uid}{canonical_email}");
    let bytes: Vec<u8> = joined.encode_utf16().map(|unit| unit as u8).collect();
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    rikka_types::hex::encode(hasher.finalize())
}
