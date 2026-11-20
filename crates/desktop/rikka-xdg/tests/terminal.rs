use std::path::Path;

use rikka_xdg::{Error, Terminals};

type Files = Vec<(&'static str, String)>;

fn terminal(exec: &str, extra: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Term\nExec={exec}\nCategories=System;TerminalEmulator;\n{extra}"
    )
}

fn layout(root: &Path) -> Terminals {
    Terminals::new(root.join("config"), root.join("data"))
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
        ("share/applications/alpha.desktop", terminal("alpha", "")),
        ("share/applications/beta.desktop", terminal("beta", "")),
        (
            "share/applications/editor.desktop",
            "[Desktop Entry]\nType=Application\nName=Editor\nExec=editor\nCategories=Utility;\n"
                .into(),
        ),
    ]
}

#[test]
fn preferred_follows_lists() {
    let cases: [(&str, Files, Option<&str>); 8] = [
        (
            "without lists the first installed one is used",
            vec![],
            Some("alpha.desktop"),
        ),
        (
            "the distribution's fallback list is honoured",
            vec![(
                "share/xdg-terminal-exec/rikka-xdg-terminals.list",
                "beta.desktop\n".into(),
            )],
            Some("beta.desktop"),
        ),
        (
            "the person's list outranks the system's",
            vec![
                ("site/xdg-terminals.list", "alpha.desktop\n".into()),
                ("config/xdg-terminals.list", "# mine\nbeta.desktop\n".into()),
            ],
            Some("beta.desktop"),
        ),
        (
            "a desktop's own list outranks the plain one beside it",
            vec![
                ("config/xdg-terminals.list", "alpha.desktop\n".into()),
                ("config/rikka-xdg-terminals.list", "beta.desktop\n".into()),
            ],
            Some("beta.desktop"),
        ),
        (
            "a missing terminal yields to the next one listed",
            vec![(
                "config/xdg-terminals.list",
                "gone.desktop\nbeta.desktop\n".into(),
            )],
            Some("beta.desktop"),
        ),
        (
            "an application that is no terminal is never chosen",
            vec![("config/xdg-terminals.list", "editor.desktop\n".into())],
            Some("alpha.desktop"),
        ),
        (
            "an excluded terminal is skipped by the fallback",
            vec![("config/xdg-terminals.list", "-alpha.desktop\n".into())],
            Some("beta.desktop"),
        ),
        (
            "the first word on an id wins over a later protection",
            vec![
                (
                    "config/xdg-terminals.list",
                    "-alpha.desktop\n-beta.desktop\n".into(),
                ),
                ("site/xdg-terminals.list", "+alpha.desktop\n".into()),
            ],
            None,
        ),
    ];
    for (case, files, expected) in cases {
        let root = tempfile::tempdir().expect("a folder");
        lay_out(root.path(), &installed());
        lay_out(root.path(), &files);
        let preferred = layout(root.path()).preferred();
        assert_eq!(
            preferred
                .as_ref()
                .map(|terminal| terminal.entry().id.as_str()),
            expected,
            "{case}"
        );
    }
}

#[test]
fn launch_spells_arguments_from_entry() {
    let cases: [(&str, &str, &[&str], Option<&str>); 4] = [
        (
            "a terminal without keys takes -e and runs in the folder",
            "",
            &["term", "-e", "htop", "-d", "5"],
            Some("/work"),
        ),
        (
            "declared keys are used and a valued key is joined",
            "X-TerminalArgExec=--\nX-TerminalArgDir=--dir=\nX-TerminalArgAppId=--class\n",
            &[
                "term",
                "--dir=/work",
                "--class",
                "top",
                "--",
                "htop",
                "-d",
                "5",
            ],
            None,
        ),
        (
            "an empty execution key passes the command as is",
            "X-TerminalArgExec=\n",
            &["term", "htop", "-d", "5"],
            Some("/work"),
        ),
        (
            "the older ExecArg key is still understood",
            "X-ExecArg=-x\n",
            &["term", "-x", "htop", "-d", "5"],
            Some("/work"),
        ),
    ];
    for (case, keys, argv, current_dir) in cases {
        let root = tempfile::tempdir().expect("a folder");
        lay_out(
            root.path(),
            &[("share/applications/term.desktop", terminal("term", keys))],
        );
        let terminal = layout(root.path()).preferred().expect("a terminal");
        let resolved = terminal
            .launch()
            .dir("/work")
            .app_id("top")
            .run(["htop", "-d", "5"])
            .resolve()
            .expect("a command");
        assert_eq!(resolved.argv, argv, "{case}");
        assert_eq!(
            resolved.current_dir.as_deref(),
            current_dir.map(Path::new),
            "{case}"
        );
    }
}

#[test]
fn prefer_puts_choice_first() {
    let root = tempfile::tempdir().expect("a folder");
    lay_out(root.path(), &installed());
    lay_out(
        root.path(),
        &[
            (
                "config/xdg-terminals.list",
                "# kept\nalpha.desktop\n-other.desktop\nbeta.desktop\n".into(),
            ),
            ("config/rikka-xdg-terminals.list", "alpha.desktop\n".into()),
        ],
    );
    let terminals = layout(root.path());

    terminals
        .prefer("beta.desktop")
        .expect("the choice is written");

    assert_eq!(
        std::fs::read_to_string(root.path().join("config/xdg-terminals.list")).expect("the list"),
        "# kept\nbeta.desktop\nalpha.desktop\n-other.desktop\n"
    );
    assert_eq!(
        terminals
            .preferred()
            .map(|terminal| terminal.entry().id.clone()),
        Some("beta.desktop".to_owned()),
        "the shadowing desktop list follows the choice"
    );
    assert!(terminals.is_chosen());

    terminals.forget_choice().expect("the choice is forgotten");

    assert!(!terminals.is_chosen());
    assert_eq!(
        std::fs::read_to_string(root.path().join("config/xdg-terminals.list")).expect("the list"),
        "# kept\n-other.desktop\n"
    );
}

#[test]
fn prefer_refuses_ids_breaking_list() {
    for id in [
        "beta",
        "beta.desktop\nevil.desktop",
        "-beta.desktop",
        "a/b.desktop",
    ] {
        let root = tempfile::tempdir().expect("a folder");
        let outcome = layout(root.path()).prefer(id);
        assert!(matches!(outcome, Err(Error::InvalidTerminal(_))), "{id:?}");
        assert!(!root.path().join("config/xdg-terminals.list").exists());
    }
}
