use std::fs;
use std::path::{Path, PathBuf};

use rikka_xdg::{Error, RecentApplication, RecentFiles};

const PAINT: RecentApplication = RecentApplication::new("rikka-paint", "rikka-paint %u");
const OTHER: RecentApplication = RecentApplication::new("other", "other %u");
const GEDIT: RecentApplication = RecentApplication::new("gedit", "gedit %u");

fn picture(folder: &Path, name: &str) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, b"pixels").expect("a picture");
    path
}

fn written_by_others(plans: &Path) -> String {
    format!(
        r#"<?xml version="1.0"?>
<xbel version="1.0"
  xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
  xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
  xmlns:kde="http://www.kde.org"
>
  <bookmark href="file://{}" added="2024-01-15T10:20:30.123456Z" modified="2024-01-16T08:00:00.000000Z" visited="2024-01-15T10:20:30.123457Z">
<title>Plans</title>
<info>
  <metadata owner="http://freedesktop.org">
    <mime:mime-type type="text/plain"/>
    <bookmark:groups>
      <bookmark:group>Editors</bookmark:group>
    </bookmark:groups>
    <bookmark:applications>
      <bookmark:application name="gedit" exec="&apos;gedit %u&apos;" modified="2024-01-16T08:00:00.000000Z" count="3"/>
    </bookmark:applications>
    <bookmark:private/>
    <kde:rating stars="4"/>
  </metadata>
  <metadata owner="http://www.kde.org"><kde:place id="7"/></metadata>
</info>
  </bookmark>
  <separator/>
</xbel>
"#,
        plans.display()
    )
}

#[test]
fn foreign_list_survives_append() {
    let folder = tempfile::tempdir().expect("a folder");
    let list = folder.path().join("recently-used.xbel");
    let plans = picture(folder.path(), "plans.txt");
    fs::write(&list, written_by_others(&plans)).expect("a list");
    let recent = RecentFiles::new(&list);

    let read = recent
        .for_application(&GEDIT, usize::MAX)
        .expect("the list");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].path, plans);
    assert_eq!(read[0].mime_type.as_deref(), Some("text/plain"));

    recent
        .add(&picture(folder.path(), "new.png"), "image/png", &PAINT)
        .expect("noted");
    let written = fs::read_to_string(&list).expect("the list");
    for entry in [
        "xmlns:kde=\"http://www.kde.org\"",
        "<kde:rating stars=\"4\"/>",
        "<metadata owner=\"http://www.kde.org\"><kde:place id=\"7\"/></metadata>",
        "<separator/>",
        "<title>Plans</title>",
        "<bookmark:group>Editors</bookmark:group>",
        "<bookmark:private/>",
        "count=\"3\"",
    ] {
        assert!(written.contains(entry), "{entry} is missing from {written}");
    }
    assert_eq!(
        recent
            .for_application(&GEDIT, usize::MAX)
            .expect("the list")
            .len(),
        1
    );
}

#[test]
fn lists_files_newest_first_per_application() {
    let folder = tempfile::tempdir().expect("a folder");
    let recent = RecentFiles::new(folder.path().join("recently-used.xbel"));
    let first = picture(folder.path(), "first.png");
    let second = picture(folder.path(), "second café #1 & <b>.png");
    let foreign = picture(folder.path(), "foreign.png");
    let gone = picture(folder.path(), "gone.png");

    recent.add(&gone, "image/png", &PAINT).expect("noted");
    recent.add(&first, "image/png", &PAINT).expect("noted");
    recent.add(&second, "image/png", &PAINT).expect("noted");
    recent.add(&first, "image/png", &PAINT).expect("noted");
    recent.add(&foreign, "image/png", &OTHER).expect("noted");
    fs::remove_file(&gone).expect("removed");

    let paths: Vec<PathBuf> = recent
        .for_application(&PAINT, usize::MAX)
        .expect("the list")
        .into_iter()
        .map(|file| file.path)
        .collect();
    assert_eq!(paths, [first, second]);
    assert_eq!(
        recent.for_application(&PAINT, 1).expect("the list").len(),
        1
    );
}

#[test]
fn rejects_and_keeps_broken_list() {
    let folder = tempfile::tempdir().expect("a folder");
    let list = folder.path().join("recently-used.xbel");
    fs::write(&list, "<xbel><bookmark").expect("a broken list");
    let recent = RecentFiles::new(&list);

    assert!(matches!(
        recent.add(&picture(folder.path(), "kept.png"), "image/png", &PAINT),
        Err(Error::RecentMalformed { .. })
    ));
    assert_eq!(
        fs::read_to_string(&list).expect("the list"),
        "<xbel><bookmark"
    );
}
