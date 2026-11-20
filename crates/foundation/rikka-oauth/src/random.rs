use rand::RngCore;

#[must_use]
pub fn random_state() -> String {
    rikka_types::hex::encode(bytes::<16>())
}

pub(crate) fn bytes<const N: usize>() -> [u8; N] {
    let mut data = [0_u8; N];
    rand::rng().fill_bytes(&mut data);
    data
}
