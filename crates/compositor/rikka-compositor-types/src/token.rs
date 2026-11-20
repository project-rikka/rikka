const BYTES: usize = 16;

pub fn unguessable() -> String {
    let mut bytes = [0u8; BYTES];
    getrandom::fill(&mut bytes).expect("the kernel provides randomness");
    rikka_types::hex::encode(bytes)
}
