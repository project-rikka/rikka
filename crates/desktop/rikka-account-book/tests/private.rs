use std::os::unix::fs::PermissionsExt;

use rikka_account_book::{read_json, write_private, write_private_json};

#[test]
fn private_file_is_replaced_owner_only() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("secret.json");
    std::fs::write(&path, b"old").unwrap();

    write_private(&path, b"new").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
}

#[test]
fn private_json_round_trips() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("session.json");
    assert_eq!(read_json::<Vec<String>>(&path).unwrap(), None);

    write_private_json(&path, &vec!["t0ken".to_owned()]).unwrap();

    assert_eq!(
        read_json::<Vec<String>>(&path).unwrap(),
        Some(vec!["t0ken".to_owned()])
    );
}
