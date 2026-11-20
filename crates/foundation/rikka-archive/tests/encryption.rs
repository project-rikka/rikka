use rikka_archive::{Archive, ArchiveWriter, CreateOptions, Error, Format};

fn write_encrypted(path: &std::path::Path, format: Format, options: CreateOptions<'_>) {
    let mut writer = ArchiveWriter::create_as(path, format, options).unwrap();
    writer.add_bytes("secret.txt", b"classified").unwrap();
    writer.finish().unwrap();
}

#[test]
fn encrypted_entries_require_password() {
    for format in [Format::Zip, Format::SevenZ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("vault.{}", format.extension()));
        write_encrypted(&path, format, CreateOptions::new().password("hunter2"));

        let mut archive = Archive::open(&path).unwrap();

        assert_eq!(archive.entries()[0].name(), "secret.txt", "{format}");
        assert!(archive.is_encrypted(), "{format} should report encryption");

        assert!(
            archive.read_entry(0).is_err(),
            "{format} read without a password must fail"
        );

        archive.set_password("wrong");
        assert!(
            archive.read_entry(0).is_err(),
            "{format} read with a wrong password must fail"
        );

        archive.set_password("hunter2");
        assert_eq!(archive.read_entry(0).unwrap(), b"classified", "{format}");
    }
}

#[test]
fn reports_missing_zip_password() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.zip");
    write_encrypted(&path, Format::Zip, CreateOptions::new().password("hunter2"));

    let mut archive = Archive::open(&path).unwrap();
    assert!(matches!(
        archive.read_entry(0),
        Err(Error::PasswordRequired)
    ));

    archive.set_password("wrong");
    assert!(matches!(archive.read_entry(0), Err(Error::WrongPassword)));
}

#[test]
fn seven_z_header_encryption_hides_listing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.7z");
    write_encrypted(
        &path,
        Format::SevenZ,
        CreateOptions::new()
            .password("hunter2")
            .encrypt_header(true),
    );

    assert!(
        Archive::open(&path).is_err(),
        "an encrypted header must not open without a password"
    );

    let mut archive = Archive::open_with_password(&path, "hunter2").unwrap();
    assert_eq!(archive.entries()[0].name(), "secret.txt");
    assert_eq!(archive.read_entry(0).unwrap(), b"classified");
}

#[test]
fn wrong_seven_z_password_yields_no_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.7z");

    let payload: Vec<u8> = (0..64 * 1024u32)
        .map(|position| position.wrapping_mul(2_654_435_761).to_le_bytes()[0])
        .collect();

    let mut writer = ArchiveWriter::create_as(
        &path,
        Format::SevenZ,
        CreateOptions::new().password("hunter2"),
    )
    .unwrap();
    writer.add_bytes("secret.bin", &payload).unwrap();
    writer.finish().unwrap();

    let mut archive = Archive::open(&path).unwrap();
    for attempt in 0..256 {
        archive.set_password(format!("wrong-{attempt}"));
        assert!(
            matches!(archive.read_entry(0), Err(Error::WrongPassword)),
            "\"wrong-{attempt}\" must be rejected as a wrong password"
        );
    }

    archive.set_password("hunter2");
    assert_eq!(archive.read_entry(0).unwrap(), payload);
}
