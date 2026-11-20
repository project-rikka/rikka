use std::io::{self, Cursor, Read, Write};

use rikka_proton_pgp::{Clock, Error, Pgp, PrivateKey, SignatureStatus, is_armored};

const BODY_LENGTH: usize = 300 * 1024;
const FAILING_AFTER: usize = 64 * 1024;

fn pgp() -> Pgp {
    Pgp::with_clock(Clock::fixed(1_700_000_000))
}

fn key() -> PrivateKey {
    pgp()
        .generate_key()
        .user_id("Rikka Stream", "stream@proton.me")
        .generate()
        .unwrap()
}

fn body() -> Vec<u8> {
    (0..BODY_LENGTH)
        .map(|index| (index * 31 % 251) as u8)
        .collect()
}

struct FailingIo {
    remaining: usize,
}

impl Read for FailingIo {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("read failed"));
        }
        let read = buffer.len().min(self.remaining);
        buffer[..read].fill(7);
        self.remaining -= read;
        Ok(read)
    }
}

impl Write for FailingIo {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("write failed"));
        }
        let count = buffer.len().min(self.remaining);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streamed_message_round_trips() {
    let pgp = pgp();
    let sender = key();
    let recipient = key();
    let body = body();

    for armored in [false, true] {
        let mut sealed = Vec::new();
        let encryption = pgp
            .encrypt()
            .to(recipient.public_key())
            .signed_by(sender.clone());
        match armored {
            true => encryption.armored_stream(Cursor::new(&body), &mut sealed),
            false => encryption.stream(Cursor::new(&body), &mut sealed),
        }
        .unwrap();
        assert_eq!(is_armored(&sealed), armored);

        let whole = pgp
            .decrypt()
            .with_key(recipient.clone())
            .verified_by(sender.public_key())
            .message(&sealed)
            .unwrap();
        assert!(whole.signature().is_valid(), "{}", whole.signature());
        assert_eq!(whole.data(), body.as_slice());

        let mut decrypted_bytes = Vec::new();
        let status = pgp
            .decrypt()
            .with_key(recipient.clone())
            .verified_by(sender.public_key())
            .stream(Cursor::new(&sealed), &mut decrypted_bytes)
            .unwrap();
        assert!(status.is_valid(), "{status}");
        assert_eq!(decrypted_bytes, body);
    }
}

#[test]
fn split_stream_separates_key_packets() {
    let pgp = pgp();
    let key = key();
    let body = body();

    let mut data_packet = Vec::new();
    let keys = pgp
        .encrypt()
        .to(key.public_key())
        .split_stream(Cursor::new(&body), &mut data_packet)
        .unwrap();

    let session_key = pgp
        .decrypt()
        .with_key(key.clone())
        .session_key(keys.key_packets())
        .unwrap();
    assert_eq!(session_key.as_bytes(), keys.session_key().as_bytes());

    let mut decrypted_bytes = Vec::new();
    pgp.decrypt()
        .with_session_key(session_key)
        .stream(Cursor::new(&data_packet), &mut decrypted_bytes)
        .unwrap();
    assert_eq!(decrypted_bytes, body);
}

#[test]
fn streamed_detached_signature_verifies() {
    let pgp = pgp();
    let key = key();
    let body = body();

    let signature = pgp
        .sign()
        .with_key(key.clone())
        .detached_stream(Cursor::new(&body))
        .unwrap();
    assert!(
        pgp.verify()
            .with_key(key.public_key())
            .detached(&body, &signature)
            .is_valid()
    );

    let armored = pgp
        .sign()
        .with_key(key.clone())
        .armored_detached_stream(Cursor::new(&body))
        .unwrap();
    assert!(armored.starts_with("-----BEGIN PGP SIGNATURE-----"));

    let whole = pgp.sign().with_key(key.clone()).detached(&body).unwrap();
    let mut altered = body.clone();
    altered[BODY_LENGTH / 2] ^= 1;

    let cases = [
        (&body, signature.as_slice(), true),
        (&body, armored.as_bytes(), true),
        (&body, whole.as_slice(), true),
        (&altered, signature.as_slice(), false),
    ];
    for (data, signature, expected_valid) in cases {
        let status = pgp
            .verify()
            .with_key(key.public_key())
            .detached_stream(Cursor::new(data), signature)
            .unwrap();
        assert_eq!(status.is_valid(), expected_valid, "{status}");
    }
}

#[test]
fn decryption_verifies_detached_signature() {
    let pgp = pgp();
    let sender = key();
    let recipient = key();
    let body = body();

    let message = pgp
        .encrypt()
        .to(recipient.public_key())
        .message(&body)
        .unwrap();
    let signature = pgp.sign().with_key(sender.clone()).detached(&body).unwrap();
    let encrypted_signature = pgp
        .encrypt()
        .to(recipient.public_key())
        .message(&signature)
        .unwrap();
    let foreign = pgp
        .sign()
        .with_key(sender.clone())
        .detached(b"another body")
        .unwrap();

    let decryption = || {
        pgp.decrypt()
            .with_key(recipient.clone())
            .verified_by(sender.public_key())
    };
    let cases = [
        (decryption().with_detached_signature(&signature), true),
        (
            decryption().with_encrypted_detached_signature(&encrypted_signature),
            true,
        ),
        (decryption().with_detached_signature(&foreign), false),
    ];
    for (decryption, expected_valid) in cases {
        let plaintext = decryption.message(&message).unwrap();
        assert_eq!(plaintext.data(), body.as_slice());
        assert_eq!(
            plaintext.signature().is_valid(),
            expected_valid,
            "{}",
            plaintext.signature()
        );
    }

    let mut decrypted_bytes = Vec::new();
    let status = decryption()
        .with_detached_signature(&signature)
        .stream(Cursor::new(&message), &mut decrypted_bytes)
        .unwrap();
    assert!(status.is_valid(), "{status}");
    assert_eq!(decrypted_bytes, body);

    let unsigned = decryption()
        .stream(Cursor::new(&message), io::sink())
        .unwrap();
    assert_eq!(unsigned, SignatureStatus::Missing);
}

#[test]
fn io_failure_reported_as_io_error() {
    let pgp = pgp();
    let key = key();
    let body = body();
    let message = pgp.encrypt().to(key.public_key()).message(&body).unwrap();

    let encrypted = pgp.encrypt().to(key.public_key()).stream(
        FailingIo {
            remaining: FAILING_AFTER,
        },
        io::sink(),
    );
    let decrypted = pgp.decrypt().with_key(key.clone()).stream(
        Cursor::new(&message),
        FailingIo {
            remaining: FAILING_AFTER,
        },
    );
    let signed = pgp.sign().with_key(key.clone()).detached_stream(FailingIo {
        remaining: FAILING_AFTER,
    });
    let verified = pgp.verify().with_key(key.public_key()).detached_stream(
        FailingIo {
            remaining: FAILING_AFTER,
        },
        b"",
    );

    assert!(matches!(encrypted, Err(Error::Io(_))), "{encrypted:?}");
    assert!(matches!(decrypted, Err(Error::Io(_))), "{decrypted:?}");
    assert!(matches!(signed, Err(Error::Io(_))), "{signed:?}");
    assert!(matches!(verified, Err(Error::Io(_))), "{verified:?}");
}
