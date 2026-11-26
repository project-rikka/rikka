use std::fs;

use rikka_age::{Decryptor, Error};

const PASSPHRASE: &str = "correct horse battery staple";
const FOLDER: &str = "crates/foundation/rikka-age/tests/oracle";

fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|at| ((at * 31 + 7) % 251) as u8).collect()
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(format!("{FOLDER}/{name}")).expect("read fixture")
}

#[test]
fn decrypts_files_written_by_the_reference_tool() {
    let cases = [
        ("small.age", b"Rikka backup oracle.\n".to_vec()),
        ("empty.age", Vec::new()),
        ("pattern-65536.age", pattern(65_536)),
        ("pattern-70000.age", pattern(70_000)),
    ];
    for (name, expected) in cases {
        let plaintext = rikka_age::decrypt(PASSPHRASE, fixture(name).as_slice())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(*plaintext, expected, "{name}");
    }
}

#[test]
fn reads_the_reference_work_factor_before_deriving() {
    let file = fixture("small.age");
    let decryptor = Decryptor::new(file.as_slice()).unwrap();
    assert_eq!(decryptor.work_factor(), 18);
    let refused = decryptor
        .max_work_factor(17)
        .decrypt(PASSPHRASE)
        .unwrap_err();
    assert!(matches!(
        refused,
        Error::WorkFactor {
            factor: 18,
            allowed: 17
        }
    ));
}

#[test]
fn refuses_the_wrong_passphrase_for_a_reference_file() {
    let refused =
        rikka_age::decrypt("battery staple", fixture("small.age").as_slice()).unwrap_err();
    assert!(matches!(refused, Error::Passphrase));
}
