use rikka_proton_drive_sync_state::{TEMPORARY_PREFIX, is_ignored, local_name};
use rikka_proton_names::NAME_LIMIT;

#[test]
fn remote_names_become_valid_local_names() {
    let long = format!("{}.txt", "長".repeat(200));
    for (remote, expected) in [
        ("notes.txt", "notes.txt".to_owned()),
        ("a/b\0c", "a_b_c".to_owned()),
        (".", "_".to_owned()),
        ("..", "__".to_owned()),
        ("", "_".to_owned()),
    ] {
        assert_eq!(local_name(remote), expected, "{remote:?}");
    }
    let shortened = local_name(&long);
    assert!(shortened.len() <= NAME_LIMIT, "{}", shortened.len());
    assert!(shortened.ends_with("長.txt"), "{shortened}");
}

#[test]
fn ignores_partial_and_temporary_files() {
    for (name, ignored) in [
        (format!("{TEMPORARY_PREFIX}1234.part"), true),
        ("~$report.docx".to_owned(), true),
        (".~lock.report.odt#".to_owned(), true),
        (".#notes.org".to_owned(), true),
        ("movie.mkv.crdownload".to_owned(), true),
        ("draft.SWP".to_owned(), true),
        (".goutputstream-ABC123".to_owned(), true),
        ("report.docx".to_owned(), false),
        (".bashrc".to_owned(), false),
        (".tmp".to_owned(), false),
    ] {
        assert_eq!(is_ignored(&name), ignored, "{name}");
    }
}
