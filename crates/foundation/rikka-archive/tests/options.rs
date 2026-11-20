use std::fs;
use std::path::Path;

use rikka_archive::{Archive, ArchiveWriter, CancelToken, Error, ExtractOptions, Flow, Overwrite};

fn sample_archive(directory: &Path) -> std::path::PathBuf {
    let path = directory.join("sample.zip");
    let mut writer = ArchiveWriter::create(&path).unwrap();
    writer.add_bytes("top/keep.txt", b"keep").unwrap();
    writer.add_bytes("top/drop.log", b"drop").unwrap();
    writer
        .add_bytes("top/nested/deep.txt", &vec![b'x'; 600_000])
        .unwrap();
    writer.finish().unwrap();
    path
}

#[test]
fn token_cancels_from_another_thread() {
    let directory = tempfile::tempdir().unwrap();
    let path = sample_archive(directory.path());

    let token = CancelToken::new();
    token.cancel();

    let result = Archive::open(&path).unwrap().extract_with(
        directory.path().join("out"),
        ExtractOptions::new().cancel_token(token),
    );
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
}

#[test]
fn honours_overwrite_policies() {
    let directory = tempfile::tempdir().unwrap();
    let path = sample_archive(directory.path());
    let out = directory.path().join("out");

    fs::create_dir_all(out.join("top")).unwrap();
    fs::write(out.join("top").join("keep.txt"), b"original").unwrap();

    let summary = Archive::open(&path)
        .unwrap()
        .extract_with(&out, ExtractOptions::new().overwrite(Overwrite::Skip))
        .unwrap();
    assert_eq!(
        fs::read(out.join("top").join("keep.txt")).unwrap(),
        b"original"
    );
    assert_eq!(summary.skipped, 1);

    let result = Archive::open(&path)
        .unwrap()
        .extract_with(&out, ExtractOptions::new().overwrite(Overwrite::Fail));
    assert!(matches!(result, Err(Error::AlreadyExists(_))), "{result:?}");

    Archive::open(&path)
        .unwrap()
        .extract_with(&out, ExtractOptions::new().overwrite(Overwrite::Replace))
        .unwrap();
    assert_eq!(fs::read(out.join("top").join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn filter_selects_written_entries() {
    let directory = tempfile::tempdir().unwrap();
    let path = sample_archive(directory.path());
    let out = directory.path().join("out");

    let summary = Archive::open(&path)
        .unwrap()
        .extract_with(
            &out,
            ExtractOptions::new().filter(|entry| entry.name().ends_with(".txt")),
        )
        .unwrap();

    assert_eq!(summary.files, 2);
    assert!(out.join("top").join("keep.txt").is_file());
    assert!(!out.join("top").join("drop.log").exists());
}

#[test]
fn metadata_round_trips() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("file.txt");
    fs::write(&source, b"content").unwrap();
    let original = fs::metadata(&source).unwrap().modified().unwrap();

    let path = directory.path().join("archive.zip");
    rikka_archive::compress(&path, [&source]).unwrap();

    let mut archive = Archive::open(&path).unwrap();
    let stored = archive.entries()[0].modified().unwrap();
    let drift = stored
        .duration_since(original)
        .unwrap_or_else(|error| error.duration());
    assert!(drift.as_secs() <= 2, "timestamp drifted by {drift:?}");

    let out = directory.path().join("out");
    archive.extract(&out).unwrap();
    let restored = fs::metadata(out.join("file.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let drift = restored
        .duration_since(stored)
        .unwrap_or_else(|error| error.duration());
    assert!(drift.as_secs() <= 2, "restored time drifted by {drift:?}");
}

#[cfg(unix)]
fn sealing(directory: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let probe = directory.join("probe");
    fs::write(&probe, b"probe").unwrap();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o600)).unwrap();
    let sealed = fs::metadata(&probe).is_err();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_file(&probe).unwrap();
    sealed
}

#[cfg(unix)]
fn tree_archive(directory: &Path) -> std::path::PathBuf {
    let source = directory.join("tree");
    fs::create_dir_all(source.join("kept")).unwrap();
    fs::write(source.join("big.bin"), vec![b'x'; 600_000]).unwrap();

    let path = directory.join("tree.zip");
    rikka_archive::compress(&path, [&source]).unwrap();
    path
}

#[cfg(unix)]
fn extract_into_a_sealing_destination(
    archive: &Path,
    out: &Path,
    options: ExtractOptions<'_>,
) -> rikka_archive::ExtractSummary {
    use std::os::unix::fs::PermissionsExt;

    let seal = |progress: rikka_archive::Progress<'_>| {
        if progress.entry_bytes > 0 {
            fs::set_permissions(out, fs::Permissions::from_mode(0o600)).unwrap();
        }
        Flow::Continue
    };
    let result = Archive::open(archive)
        .unwrap()
        .extract_with(out, options.on_progress(seal));
    fs::set_permissions(out, fs::Permissions::from_mode(0o700)).unwrap();
    result.unwrap()
}

#[cfg(unix)]
#[test]
fn reports_unapplied_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = tree_archive(directory.path());
    let out = directory.path().join("out");
    fs::create_dir_all(&out).unwrap();

    if !sealing(&out) {
        return;
    }

    let summary = extract_into_a_sealing_destination(&path, &out, ExtractOptions::new());

    assert_eq!(summary.files, 1);
    assert_eq!(
        fs::read(out.join("tree").join("big.bin")).unwrap().len(),
        600_000,
        "the contents are what an extraction is for"
    );

    let rejected: Vec<String> = summary
        .metadata
        .rejected
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(!summary.metadata.is_empty());
    assert_eq!(summary.metadata.failed, rejected.len());
    assert!(
        rejected
            .iter()
            .any(|line| line.contains("big.bin") && line.contains("permissions")),
        "{rejected:?}"
    );
    assert!(
        rejected
            .iter()
            .any(|line| line.contains("big.bin") && line.contains("timestamp")),
        "{rejected:?}"
    );
    assert!(
        rejected
            .iter()
            .any(|line| line.contains("kept") && line.contains("timestamp")),
        "the deferred directory times are answered for too: {rejected:?}"
    );
}
