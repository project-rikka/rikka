use std::path::Path;

use rikka_xdg::{DesktopEntry, Sources};

const TERMINAL: &str = "\
[Desktop Entry]
Type=Application
Name=Terminal
GenericName=Terminal Emulator
Comment=Use the command line
Keywords=shell;prompt;command line;
Categories=System;TerminalEmulator;
MimeType=x-scheme-handler/ssh;
Exec=foot %U
Icon=utilities-terminal
";

fn parsed(text: &str) -> Option<DesktopEntry> {
    DesktopEntry::parse("test.desktop", Path::new("/x/test.desktop"), text, &[])
}

#[test]
fn reads_application_entry() {
    let entry = parsed(TERMINAL).expect("a listable application");
    assert_eq!(entry.name, "Terminal");
    assert_eq!(entry.generic_name.as_deref(), Some("Terminal Emulator"));
    assert_eq!(entry.keywords, ["shell", "prompt", "command line"]);
    assert_eq!(entry.categories, ["System", "TerminalEmulator"]);
    assert_eq!(entry.exec, "foot %U");
    assert!(entry.handles("x-scheme-handler/ssh"));
    assert!(!entry.terminal);
    assert!(!entry.no_display);
}

#[test]
fn non_application_is_not_entry() {
    assert!(parsed("[Desktop Entry]\nType=Link\nName=Home\nURL=file:///\n").is_none());
    assert!(
        parsed("[Desktop Entry]\nType=Application\nName=X\n").is_none(),
        "no Exec"
    );
    assert!(
        parsed("[Desktop Entry]\nName=X\nExec=x\n").is_none(),
        "no Type"
    );
    assert!(parsed("Exec=x\n").is_none(), "no group");
}

#[test]
fn hidden_entry_removed_nodisplay_kept() {
    assert!(parsed(&format!("{TERMINAL}Hidden=true\n")).is_none());

    let unlisted = parsed(&format!("{TERMINAL}NoDisplay=true\n")).expect("still installed");
    assert!(unlisted.no_display, "the menu decides, not the parser");
}

#[test]
fn uses_matching_translation() {
    let text = "\
[Desktop Entry]
Type=Application
Exec=foot
Name=Terminal
Name[ja]=端末
Comment=Command line
";
    let path = Path::new("/x/t.desktop");
    let japanese = DesktopEntry::parse("t.desktop", path, text, &["ja".to_owned()]).unwrap();
    assert_eq!(japanese.name, "端末");
    assert_eq!(
        japanese.comment.as_deref(),
        Some("Command line"),
        "a key with no translation falls back to the plain one"
    );
    assert_eq!(
        DesktopEntry::parse("t.desktop", path, text, &["fr".to_owned()])
            .unwrap()
            .name,
        "Terminal"
    );
}

#[test]
fn entry_reports_desktops() {
    let gnome_only = parsed(&format!("{TERMINAL}OnlyShowIn=GNOME;\n")).unwrap();
    assert!(!gnome_only.shows_in(&["KDE".to_owned()]));
    assert!(gnome_only.shows_in(&["GNOME".to_owned()]));
    assert!(
        !gnome_only.shows_in(&[]),
        "an entry that named where it belongs belongs nowhere else"
    );

    let not_rikka = parsed(&format!("{TERMINAL}NotShowIn=Rikka;\n")).unwrap();
    assert!(!not_rikka.shows_in(&["Rikka".to_owned()]));
    assert!(not_rikka.shows_in(&["GNOME".to_owned()]));
}

#[test]
fn entry_reports_missing_program() {
    assert!(
        !parsed(&format!("{TERMINAL}TryExec=/nonexistent/foot\n"))
            .unwrap()
            .is_installed()
    );
    assert!(
        parsed(&format!("{TERMINAL}TryExec=sh\n"))
            .unwrap()
            .is_installed()
    );
    assert!(
        parsed(TERMINAL).unwrap().is_installed(),
        "silence is consent"
    );
}

#[test]
fn replaced_link_marks_directory_stale() {
    let root = tempfile::tempdir().expect("a directory to work in");
    let installed = root.path().join("one");
    let then_installed = root.path().join("two");
    let profile = root.path().join("profile");
    std::fs::create_dir(&installed).expect("the first generation");
    std::fs::create_dir(&then_installed).expect("the second");
    std::os::unix::fs::symlink(&installed, &profile).expect("the link a session reads");

    let sources = Sources::from_dirs(std::slice::from_ref(&profile));
    assert!(!sources.is_stale(std::slice::from_ref(&profile)));

    std::fs::remove_file(&profile).expect("the link is replaced, not written through");
    std::os::unix::fs::symlink(&then_installed, &profile).expect("the next generation");

    assert!(sources.is_stale(std::slice::from_ref(&profile)));
}
