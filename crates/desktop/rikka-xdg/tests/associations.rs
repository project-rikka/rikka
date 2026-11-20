use std::path::Path;

use rikka_mime::MimeType;
use rikka_xdg::{Error, MimeApps};

const WEB: &str = "x-scheme-handler/https";

type Files = Vec<(&'static str, String)>;

fn mime(essence: &str) -> MimeType {
    MimeType::parse(essence).expect("a media type")
}

fn application(types: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName=App\nExec=app %u\nMimeType={types}\n")
}

fn hidden() -> String {
    "[Desktop Entry]\nType=Application\nName=App\nExec=app %u\nHidden=true\n".to_owned()
}

fn layout(root: &Path) -> MimeApps {
    MimeApps::new(root.join("config"), root.join("data"))
        .config_dirs([root.join("site")])
        .data_dirs([root.join("share")])
        .desktops(["Rikka"])
}

fn lay_out(root: &Path, files: &[(&str, String)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(path, text).expect("the file");
    }
}

fn installed() -> Files {
    vec![
        (
            "share/applications/one.desktop",
            application("x-scheme-handler/https;"),
        ),
        (
            "share/applications/two.desktop",
            application("x-scheme-handler/https;"),
        ),
        (
            "share/applications/plain.desktop",
            application("text/plain;"),
        ),
    ]
}

#[test]
fn default_follows_lookup_order() {
    let cases: [(&str, Files, Option<&str>); 10] = [
        (
            "the distribution's choice stands alone",
            vec![(
                "share/applications/mimeapps.list",
                "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
            )],
            Some("two.desktop"),
        ),
        (
            "the person's list outranks the distribution's",
            vec![
                (
                    "share/applications/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
                ),
                (
                    "config/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=one.desktop\n".into(),
                ),
            ],
            Some("one.desktop"),
        ),
        (
            "a desktop's own list outranks the plain one beside it",
            vec![
                (
                    "config/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=one.desktop\n".into(),
                ),
                (
                    "config/rikka-mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
                ),
            ],
            Some("two.desktop"),
        ),
        (
            "another desktop's list is not read",
            vec![
                (
                    "config/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=one.desktop\n".into(),
                ),
                (
                    "config/gnome-mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
                ),
            ],
            Some("one.desktop"),
        ),
        (
            "a program that is gone yields to the next in its list",
            vec![(
                "config/mimeapps.list",
                "[Default Applications]\nx-scheme-handler/https=gone.desktop;two.desktop;\n".into(),
            )],
            Some("two.desktop"),
        ),
        (
            "a default that cannot open the type yields to the next list",
            vec![
                (
                    "config/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=plain.desktop\n".into(),
                ),
                (
                    "site/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
                ),
            ],
            Some("two.desktop"),
        ),
        (
            "an added association lets a default open the type",
            vec![(
                "config/mimeapps.list",
                "[Default Applications]\nx-scheme-handler/https=plain.desktop\n\
                 [Added Associations]\nx-scheme-handler/https=plain.desktop;\n"
                    .into(),
            )],
            Some("plain.desktop"),
        ),
        (
            "without a default the most preferred association opens it",
            vec![(
                "site/mimeapps.list",
                "[Added Associations]\nx-scheme-handler/https=two.desktop;one.desktop;\n".into(),
            )],
            Some("two.desktop"),
        ),
        (
            "a removed association is no longer offered",
            vec![(
                "config/mimeapps.list",
                "[Removed Associations]\nx-scheme-handler/https=one.desktop;two.desktop;\n".into(),
            )],
            None,
        ),
        (
            "a hidden entry deletes the one it shadows",
            vec![
                ("data/applications/two.desktop", hidden()),
                (
                    "config/mimeapps.list",
                    "[Default Applications]\nx-scheme-handler/https=two.desktop;\n".into(),
                ),
            ],
            Some("one.desktop"),
        ),
    ];
    for (case, files, expected) in cases {
        let root = tempfile::tempdir().expect("a folder");
        lay_out(root.path(), &installed());
        lay_out(root.path(), &files);
        let associations = layout(root.path()).read();
        assert_eq!(
            associations
                .default_for(&mime(WEB))
                .map(|entry| entry.id.as_str()),
            expected,
            "{case}"
        );
    }
}

#[test]
fn set_default_keeps_other_entries() {
    let cases: [(&str, Option<&str>, &str); 3] = [
        (
            "a missing list is written fresh",
            None,
            "[Default Applications]\n\
             x-scheme-handler/https=one.desktop;\n\
             text/html=one.desktop;\n\
             \n\
             [Added Associations]\n\
             x-scheme-handler/https=one.desktop;\n\
             text/html=one.desktop;\n",
        ),
        (
            "comments, other keys and other groups stay where they were",
            Some(
                "# chosen by hand\n\
                 [Default Applications]\n\
                 image/png=viewer.desktop\n\
                 x-scheme-handler/https = two.desktop;one.desktop\n\
                 \n\
                 [Removed Associations]\n\
                 x-scheme-handler/https=one.desktop;three.desktop;\n\
                 text/html=one.desktop;\n\
                 \n\
                 [Something Else]\n\
                 key=value\n",
            ),
            "# chosen by hand\n\
             [Default Applications]\n\
             image/png=viewer.desktop\n\
             x-scheme-handler/https=one.desktop;two.desktop;\n\
             text/html=one.desktop;\n\
             \n\
             [Removed Associations]\n\
             x-scheme-handler/https=three.desktop;\n\
             \n\
             [Something Else]\n\
             key=value\n\
             \n\
             [Added Associations]\n\
             x-scheme-handler/https=one.desktop;\n\
             text/html=one.desktop;\n",
        ),
        (
            "a key written twice ends up written once",
            Some(
                "[Default Applications]\n\
                 text/html=two.desktop\n\
                 [Default Applications]\n\
                 text/html=three.desktop\n\
                 [Added Associations]\n\
                 x-scheme-handler/https=two.desktop;\n\
                 text/html=one.desktop;three.desktop;\n",
            ),
            "[Default Applications]\n\
             [Default Applications]\n\
             text/html=one.desktop;three.desktop;\n\
             x-scheme-handler/https=one.desktop;\n\
             [Added Associations]\n\
             x-scheme-handler/https=one.desktop;two.desktop;\n\
             text/html=one.desktop;three.desktop;\n",
        ),
    ];
    for (case, before, after) in cases {
        let root = tempfile::tempdir().expect("a folder");
        lay_out(root.path(), &installed());
        if let Some(before) = before {
            lay_out(root.path(), &[("config/mimeapps.list", before.to_owned())]);
        }
        let apps = layout(root.path());
        apps.set_default("one.desktop", &[mime(WEB), mime("text/html")])
            .expect("the default is written");
        let written =
            std::fs::read_to_string(root.path().join("config/mimeapps.list")).expect("the list");
        assert_eq!(written, after, "{case}");
        assert_eq!(
            apps.read()
                .default_for(&mime(WEB))
                .map(|entry| entry.id.as_str()),
            Some("one.desktop"),
            "{case}: it opens the type afterwards"
        );
    }
}

#[test]
fn set_default_overrides_desktop_specific() {
    let root = tempfile::tempdir().expect("a folder");
    lay_out(root.path(), &installed());
    lay_out(
        root.path(),
        &[
            (
                "config/rikka-mimeapps.list",
                "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
            ),
            (
                "site/mimeapps.list",
                "[Removed Associations]\nx-scheme-handler/https=one.desktop;\n".into(),
            ),
        ],
    );
    let apps = layout(root.path());

    apps.set_default("one.desktop", &[mime(WEB)])
        .expect("the default is written");

    assert_eq!(
        apps.read()
            .default_for(&mime(WEB))
            .map(|entry| entry.id.as_str()),
        Some("one.desktop")
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("config/rikka-mimeapps.list")).expect("the list"),
        "[Default Applications]\nx-scheme-handler/https=one.desktop;two.desktop;\n",
        "the earlier choice remains the fallback"
    );
}

#[test]
fn rejects_values_breaking_list() {
    let refused = [
        "one",
        "one.desktop\n[Default Applications]\nx=evil.desktop",
        "one;evil.desktop",
    ];
    for application in refused {
        let root = tempfile::tempdir().expect("a folder");
        let outcome = layout(root.path()).set_default(application, &[mime(WEB)]);
        assert!(
            matches!(outcome, Err(Error::InvalidAssociation(_))),
            "{application:?}"
        );
        assert!(!root.path().join("config/mimeapps.list").exists());
    }
}

#[test]
fn forget_default_returns_to_the_system_choice() {
    let root = tempfile::tempdir().expect("a folder");
    lay_out(root.path(), &installed());
    lay_out(
        root.path(),
        &[(
            "share/applications/mimeapps.list",
            "[Default Applications]\nx-scheme-handler/https=two.desktop\n".into(),
        )],
    );
    let apps = layout(root.path());
    apps.set_default("one.desktop", &[mime(WEB)])
        .expect("the default is written");
    lay_out(
        root.path(),
        &[(
            "config/rikka-mimeapps.list",
            "[Default Applications]\nx-scheme-handler/https=one.desktop\nimage/png=viewer.desktop\n"
                .into(),
        )],
    );
    assert!(apps.read().is_chosen(&mime(WEB)));

    apps.forget_default(&[mime(WEB)])
        .expect("the choice is forgotten");

    let associations = apps.read();
    assert!(!associations.is_chosen(&mime(WEB)));
    assert_eq!(
        associations
            .default_for(&mime(WEB))
            .map(|entry| entry.id.as_str()),
        Some("two.desktop")
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("config/rikka-mimeapps.list")).expect("the list"),
        "[Default Applications]\nimage/png=viewer.desktop\n",
        "other types stay chosen"
    );
}

#[test]
fn handlers_and_types_follow_associations() {
    let root = tempfile::tempdir().expect("a folder");
    lay_out(root.path(), &installed());
    lay_out(
        root.path(),
        &[(
            "config/mimeapps.list",
            "[Default Applications]\nx-scheme-handler/https=two.desktop\n\
             [Added Associations]\ntext/plain=one.desktop;\n\
             [Removed Associations]\nx-scheme-handler/https=one.desktop;\n"
                .into(),
        )],
    );
    let associations = layout(root.path()).read();
    let ids = |essence: &str| -> Vec<String> {
        associations
            .handlers(&mime(essence))
            .into_iter()
            .map(|entry| entry.id.clone())
            .collect()
    };

    assert_eq!(ids(WEB), ["two.desktop"]);
    assert_eq!(ids("text/plain"), ["one.desktop", "plain.desktop"]);
    assert_eq!(associations.types(), [mime("text/plain"), mime(WEB)]);
}

#[test]
fn supertypes_answer_for_unclaimed_subtype() {
    let root = tempfile::tempdir().expect("a folder");
    lay_out(root.path(), &installed());
    let associations = layout(root.path()).read();
    let ancestry = ["text/x-rust", "text/plain", "application/octet-stream"].map(mime);

    assert!(associations.default_for(&mime("text/x-rust")).is_none());
    assert_eq!(
        associations
            .default_for_any(&ancestry)
            .map(|entry| entry.id.as_str()),
        Some("plain.desktop")
    );
    let handlers: Vec<&str> = associations
        .handlers_for_any(&["text/plain", "text/x-rust", "text/plain"].map(mime))
        .into_iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(handlers, ["plain.desktop"]);
}
