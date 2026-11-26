use std::io::{Read, Write};

use rikka_age::{Decryptor, Encryptor, Error};

const PASSPHRASE: &str = "a long passphrase";
const FAST: u8 = 4;
const CHUNK: usize = 64 * 1024;

type Expected = fn(&Error) -> bool;

fn encrypted(plaintext: &[u8]) -> Vec<u8> {
    let mut writer = Encryptor::with_passphrase(PASSPHRASE)
        .work_factor(FAST)
        .wrap(Vec::new())
        .unwrap();
    for piece in plaintext.chunks(1000) {
        writer.write_all(piece).unwrap();
    }
    writer.finish().unwrap()
}

fn decrypted(ciphertext: &[u8]) -> rikka_age::Result<Vec<u8>> {
    rikka_age::decrypt(PASSPHRASE, ciphertext).map(|plaintext| plaintext.to_vec())
}

fn payload_start(ciphertext: &[u8]) -> usize {
    let footer = ciphertext
        .windows(4)
        .position(|window| window == b"\n---")
        .unwrap();
    let end = ciphertext[footer + 1..]
        .iter()
        .position(|byte| *byte == b'\n')
        .unwrap();
    footer + 1 + end + 1 + 16
}

#[test]
fn round_trips_every_chunk_boundary() {
    for length in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK, 2 * CHUNK + 5] {
        let plaintext: Vec<u8> = (0..length).map(|at| (at % 253) as u8).collect();
        assert_eq!(
            decrypted(&encrypted(&plaintext)).unwrap(),
            plaintext,
            "{length}"
        );
    }
}

#[test]
fn writes_a_single_scrypt_stanza_header() {
    let ciphertext = encrypted(b"hello");
    let text = String::from_utf8_lossy(&ciphertext);
    let mut lines = text.split('\n');
    assert_eq!(lines.next(), Some("age-encryption.org/v1"));
    let stanza: Vec<&str> = lines.next().unwrap().split(' ').collect();
    assert_eq!(stanza[..2], ["->", "scrypt"]);
    assert_eq!(stanza[2].len(), 22);
    assert_eq!(stanza[3], FAST.to_string());
    assert_eq!(lines.next().unwrap().len(), 43);
    assert!(lines.next().unwrap().starts_with("--- "));
}

#[test]
fn streams_through_the_reader() {
    let plaintext = vec![7u8; 3 * CHUNK + 11];
    let ciphertext = encrypted(&plaintext);
    let mut reader = Decryptor::new(ciphertext.as_slice())
        .unwrap()
        .decrypt(PASSPHRASE)
        .unwrap();
    let mut read = Vec::new();
    let mut piece = [0u8; 4096];
    loop {
        let count = reader.read(&mut piece).unwrap();
        if count == 0 {
            break;
        }
        read.extend_from_slice(&piece[..count]);
    }
    assert_eq!(read, plaintext);
}

#[test]
fn refuses_damaged_files() {
    let plaintext = vec![1u8; CHUNK + 100];
    let ciphertext = encrypted(&plaintext);
    let start = payload_start(&ciphertext);

    let mut header = ciphertext.clone();
    header[33] ^= 1;
    let mut payload = ciphertext.clone();
    payload[start + 10] ^= 1;
    let mut nonce = ciphertext.clone();
    nonce[start - 1] ^= 1;
    let cut_at_chunk = ciphertext[..start + CHUNK + 16].to_vec();
    let short = ciphertext[..ciphertext.len() - 1].to_vec();
    let mut trailing = ciphertext.clone();
    trailing.push(0);
    let headless = ciphertext[..start - 16].to_vec();

    let cases: [(&str, Vec<u8>, Expected); 7] = [
        ("header", header, |error| {
            matches!(
                error,
                Error::Tampered | Error::Passphrase | Error::Header(_)
            )
        }),
        ("payload", payload, |error| matches!(error, Error::Payload)),
        ("nonce", nonce, |error| matches!(error, Error::Payload)),
        ("cut at a chunk", cut_at_chunk, |error| {
            matches!(error, Error::Payload)
        }),
        ("short", short, |error| matches!(error, Error::Payload)),
        ("trailing", trailing, |error| {
            matches!(error, Error::Payload)
        }),
        ("headless", headless, |error| {
            matches!(error, Error::Truncated)
        }),
    ];
    for (name, damaged, expected) in cases {
        let error = decrypted(&damaged).unwrap_err();
        assert!(expected(&error), "{name}: {error}");
    }
}

#[test]
fn refuses_what_is_not_a_passphrase_file() {
    let recipients = b"age-encryption.org/v1\n-> X25519 c2FsdA\nYm9keQ\n--- AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n";
    let armored = b"-----BEGIN AGE ENCRYPTED FILE-----\n";
    let cases: [(&str, &[u8], Expected); 3] = [
        ("recipients", recipients, |error| {
            matches!(error, Error::NotPassphrase)
        }),
        ("armored", armored, |error| matches!(error, Error::Armored)),
        ("text", b"hello\n", |error| matches!(error, Error::NotAge)),
    ];
    for (name, file, expected) in cases {
        let error = Decryptor::new(file).unwrap_err();
        assert!(expected(&error), "{name}: {error}");
    }
}

#[test]
fn refuses_the_wrong_passphrase() {
    let ciphertext = encrypted(b"secret");
    let error = rikka_age::decrypt("another", ciphertext.as_slice()).unwrap_err();
    assert!(matches!(error, Error::Passphrase));
}
