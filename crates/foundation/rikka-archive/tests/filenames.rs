use std::fs;
use std::path::Path;

use rikka_archive::Archive;

fn zip_with_raw_name(path: &Path, name_bytes: &[u8], contents: &[u8]) {
    let crc = crc32(contents);
    let size = u32::try_from(contents.len()).unwrap();
    let name_len = u16::try_from(name_bytes.len()).unwrap();

    let mut archive = Vec::new();

    archive.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    archive.extend_from_slice(&10u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0x21u16.to_le_bytes());
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&size.to_le_bytes());
    archive.extend_from_slice(&size.to_le_bytes());
    archive.extend_from_slice(&name_len.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(name_bytes);
    archive.extend_from_slice(contents);

    let central_offset = u32::try_from(archive.len()).unwrap();
    archive.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    archive.extend_from_slice(&20u16.to_le_bytes());
    archive.extend_from_slice(&10u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0x21u16.to_le_bytes());
    archive.extend_from_slice(&crc.to_le_bytes());
    archive.extend_from_slice(&size.to_le_bytes());
    archive.extend_from_slice(&size.to_le_bytes());
    archive.extend_from_slice(&name_len.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(name_bytes);

    let central_size = u32::try_from(archive.len()).unwrap() - central_offset;
    archive.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&1u16.to_le_bytes());
    archive.extend_from_slice(&1u16.to_le_bytes());
    archive.extend_from_slice(&central_size.to_le_bytes());
    archive.extend_from_slice(&central_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());

    fs::write(path, archive).unwrap();
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(data);
    crc.sum()
}

#[test]
fn shift_jis_names_are_decoded() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.zip");

    let name = "新しいフォルダー/写真.jpg";
    let (raw, _, had_errors) = encoding_rs::SHIFT_JIS.encode(name);
    assert!(!had_errors);
    zip_with_raw_name(&path, &raw, b"jpeg data");

    let mut archive = Archive::open(&path).unwrap();
    assert_eq!(archive.entries()[0].name(), name);
    assert_eq!(archive.read_entry(0).unwrap(), b"jpeg data");

    let out = directory.path().join("out");
    archive.extract(&out).unwrap();
    assert!(out.join("新しいフォルダー").join("写真.jpg").is_file());
}

#[test]
fn names_round_trip_in_every_format() {
    let name = "資料/報告書 (最終版).txt";

    for format in [
        rikka_archive::Format::Zip,
        rikka_archive::Format::SevenZ,
        rikka_archive::Format::TarGzip,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(format!("jp.{}", format.extension()));

        let mut writer = rikka_archive::ArchiveWriter::create_as(
            &path,
            format,
            rikka_archive::CreateOptions::new(),
        )
        .unwrap();
        writer.add_bytes(name, b"contents").unwrap();
        writer.finish().unwrap();

        let mut archive = Archive::open(&path).unwrap();
        assert_eq!(archive.entries()[0].name(), name, "{format}");
        assert_eq!(archive.read_entry(0).unwrap(), b"contents", "{format}");
    }
}
