use music_spotify_wire::Shannon;

const KEY: &[u8] = b"test key 128bits";
const NONCE: u32 = 0;
const SAMPLE: usize = 100;
const LATER: usize = 1_000_000;
const ITERATIONS: usize = 999_999;

const KEYSTREAM: [u8; 20] = [
    0x4d, 0x7e, 0xd3, 0x9c, 0xb6, 0x95, 0xd9, 0x6a, 0xcf, 0x52, 0x97, 0x70, 0xec, 0x7d, 0xcc, 0xbe,
    0xae, 0x2b, 0x6f, 0x8c,
];
const LATER_KEYSTREAM: [u8; 20] = [
    0x27, 0x01, 0x9f, 0xc8, 0x84, 0xbb, 0x09, 0x05, 0xea, 0x08, 0xc9, 0xb5, 0x5f, 0x20, 0x7b, 0x5d,
    0x34, 0x80, 0xb4, 0xa3,
];
const MAC: [u8; 20] = [
    0x00, 0x13, 0x88, 0xe9, 0x6b, 0xa7, 0x8e, 0x74, 0x4e, 0xb0, 0xb0, 0x30, 0x44, 0x25, 0xc0, 0x90,
    0x36, 0xdc, 0x80, 0x1a,
];
const ITERATED: [u8; 20] = [
    0x4e, 0x00, 0x9e, 0x3f, 0x99, 0x3e, 0x3a, 0x1e, 0xb9, 0xcb, 0x28, 0x11, 0xa2, 0xe9, 0x09, 0x69,
    0xa8, 0x9e, 0x1e, 0xf3,
];

fn keyed() -> Shannon {
    let mut cipher = Shannon::new(KEY);
    cipher.set_nonce(NONCE);
    cipher
}

fn mac(cipher: &mut Shannon) -> [u8; 20] {
    let mut mac = [0u8; 20];
    cipher.finish(&mut mac);
    mac
}

#[test]
fn keystream_matches_reference_vectors() {
    let mut zeros = vec![0u8; LATER + SAMPLE];
    keyed().encrypt(&mut zeros);
    assert_eq!(zeros[..KEYSTREAM.len()], KEYSTREAM);
    assert_eq!(zeros[LATER..LATER + LATER_KEYSTREAM.len()], LATER_KEYSTREAM);
}

#[test]
fn encryption_and_mac_match_reference_whole_or_bytewise() {
    for bytewise in [false, true] {
        let mut sealed = [0u8; SAMPLE];
        let mut sealing = keyed();
        match bytewise {
            false => sealing.encrypt(&mut sealed),
            true => sealed.chunks_mut(1).for_each(|byte| sealing.encrypt(byte)),
        }
        assert_eq!(sealed[..KEYSTREAM.len()], KEYSTREAM, "bytewise: {bytewise}");
        assert_eq!(mac(&mut sealing), MAC, "bytewise: {bytewise}");

        let mut opening = keyed();
        match bytewise {
            false => opening.decrypt(&mut sealed),
            true => sealed.chunks_mut(1).for_each(|byte| opening.decrypt(byte)),
        }
        assert_eq!(sealed, [0u8; SAMPLE], "bytewise: {bytewise}");
        assert_eq!(mac(&mut opening), MAC, "bytewise: {bytewise}");
    }
}

#[test]
fn iterated_keying_matches_reference_vector() {
    let mut buffer = KEYSTREAM;
    for _ in 0..ITERATIONS {
        let mut keystream = [0u8; 20];
        Shannon::new(&buffer).encrypt(&mut keystream);
        for (byte, key) in buffer.iter_mut().zip(keystream) {
            *byte ^= key;
        }
    }
    assert_eq!(buffer, ITERATED);
}
