use rikka_session::toplevels::{self, Bounds, Toplevel};
use rikka_session::wallpaper::{self, DisplayedWallpaper, Preview, Thumbnail, WallpaperEntry};
use serde_json::json;

use crate::support::{RecordedCall, Recorder, Session};

fn window(id: &str, title: &str, focused: bool) -> Toplevel {
    Toplevel {
        id: id.to_owned(),
        app_id: "org.rikka.Files".to_owned(),
        title: title.to_owned(),
        output: "DP-1".to_owned(),
        workspace: 2,
        focused,
        bounds: Bounds::default(),
        on_screen: true,
        minimized: false,
        maximized: false,
        tiled: false,
    }
}

fn recorded(method: &str, params: serde_json::Value) -> RecordedCall {
    RecordedCall {
        method: method.to_owned(),
        params,
    }
}

#[test]
fn window_commands_name_target_window() {
    let mut session = Session::start();
    let windows = Recorder::new(toplevels::PROTOCOL)
        .answer(
            toplevels::STATE,
            toplevels::State {
                toplevels: vec![
                    window("toplevel-3", "Home", true),
                    window("toplevel-12", "ダウンロード", false),
                ],
            },
        )
        .accept(toplevels::FOCUS)
        .accept(toplevels::CLOSE)
        .accept(toplevels::MINIMIZE)
        .accept(toplevels::MOVE);
    session.serve("compositor", windows.clone());

    let listed = session.expect_success(&["window", "list"]);
    let rows: Vec<Vec<&str>> = listed
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec!["ID", "APP", "OUTPUT", "WORKSPACE", "STATE", "TITLE"],
            vec![
                "toplevel-3",
                "org.rikka.Files",
                "DP-1",
                "2",
                "focused",
                "Home"
            ],
            vec![
                "toplevel-12",
                "org.rikka.Files",
                "DP-1",
                "2",
                "shown",
                "ダウンロード"
            ],
        ]
    );

    let listed: serde_json::Value =
        serde_json::from_str(&session.expect_success(&["window", "list", "--json"])).unwrap();
    assert_eq!(listed[1]["title"], "ダウンロード");

    session.expect_success(&["window", "focus", "toplevel-12"]);
    session.expect_success(&["window", "minimize", "toplevel-12"]);
    session.expect_success(&["window", "move", "toplevel-12", "4"]);
    session.expect_success(&["window", "close", "toplevel-3"]);
    let requests: Vec<RecordedCall> = windows
        .calls()
        .into_iter()
        .filter(|recorded| recorded.method != toplevels::STATE)
        .collect();
    assert_eq!(
        requests,
        [
            recorded(toplevels::FOCUS, json!({ "id": "toplevel-12" })),
            recorded(toplevels::MINIMIZE, json!({ "id": "toplevel-12" })),
            recorded(
                toplevels::MOVE,
                json!({ "id": "toplevel-12", "workspace": 4 })
            ),
            recorded(toplevels::CLOSE, json!({ "id": "toplevel-3" })),
        ]
    );
}

#[test]
fn wallpaper_set_requires_library_entry() {
    let mut session = Session::start();
    let library = Recorder::new(wallpaper::PROTOCOL)
        .answer(
            wallpaper::LIST,
            vec![WallpaperEntry {
                id: "dunes.jpg".to_owned(),
                name: "Dunes".to_owned(),
                preview: Preview::Still {
                    thumbnail: Thumbnail::None,
                },
            }],
        )
        .answer(
            wallpaper::SHOWING,
            Some(DisplayedWallpaper {
                id: "dunes.jpg".to_owned(),
                file: "/wallpapers/dunes.jpg".into(),
            }),
        )
        .accept(wallpaper::REFRESH);
    session.serve("shell", library.clone());

    let listed = session.expect_success(&["wallpaper", "list"]);
    assert!(
        listed
            .lines()
            .any(|line| line.split_whitespace().eq(["dunes.jpg", "Dunes"])),
        "{listed}"
    );
    assert_eq!(
        session.expect_success(&["wallpaper", "current"]).trim(),
        "dunes.jpg"
    );

    session.expect_success(&["wallpaper", "set", "dunes.jpg"]);
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.appearance", "wallpaper"])
            .trim(),
        "\"dunes.jpg\""
    );

    let absent = session.expect_failure(&["wallpaper", "set", "sea.png"]);
    assert!(absent.contains("sea.png"), "{absent}");
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.appearance", "wallpaper"])
            .trim(),
        "\"dunes.jpg\"",
        "a expect_failure wallpaper leaves the choice alone"
    );

    session.expect_success(&["wallpaper", "clear"]);
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.appearance", "wallpaper"])
            .trim(),
        "\"\""
    );

    session.expect_success(&["wallpaper", "refresh"]);
    assert!(
        library
            .calls()
            .iter()
            .any(|recorded| recorded.method == wallpaper::REFRESH)
    );
}
