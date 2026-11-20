use std::path::PathBuf;

use rikka_session::activation::{ActivationRequest, ActivationToken};
use rikka_session::toplevels::{self, Bounds};
use rikka_session::wallpaper::{Preview, Thumbnail, WallpaperEntry};
use rikka_session::workspaces::{self, COUNT, Show};
use serde_json::json;

#[test]
fn windows_payload_round_trip() {
    let payload = json!({
        "toplevels": [
            {
                "id": "toplevel-3",
                "app_id": "org.rikka.Files",
                "title": "Home",
                "output": "DP-1",
                "workspace": 2,
                "focused": true,
                "bounds": { "x": 100, "y": 40, "width": 1280, "height": 720 },
                "on_screen": true,
            },
            {
                "id": "toplevel-1",
                "app_id": "org.rikka.Terminal",
                "title": "Terminal",
                "output": "DP-1",
                "workspace": 1,
                "focused": false,
                "bounds": { "x": 0, "y": 0, "width": 800, "height": 600 },
                "on_screen": false,
                "minimized": true,
            },
        ],
    });
    let state: toplevels::State =
        serde_json::from_value(payload).expect("payload should match the session shape");

    let files = &state.toplevels[0];
    assert_eq!(files.app_id, "org.rikka.Files");
    assert_eq!(files.workspace, 2);
    assert!(files.focused && files.on_screen && !files.minimized);
    assert_eq!(
        files.bounds,
        Bounds {
            x: 100,
            y: 40,
            width: 1280,
            height: 720
        }
    );
    let terminal = &state.toplevels[1];
    assert!(terminal.minimized && !terminal.on_screen);
}

#[test]
fn workspaces_require_names() {
    let payload = json!({
        "count": 10,
        "displays": [{
            "output": "DP-1",
            "showing": 3,
            "holding": [true, false, true, false, false, false, false, false, false, false],
        }],
    });
    let state: workspaces::State =
        serde_json::from_value(payload).expect("payload should match the session shape");
    assert_eq!(state.count, COUNT);
    let display = state.display("DP-1").expect("display should exist");
    assert_eq!(display.showing, 3);
    assert!(display.holding[0]);
    assert!(state.display("HDMI-A-1").is_none());

    let short = json!({
        "count": 10,
        "displays": [{ "output": "DP-1", "showing": 1, "holding": [true, false] }],
    });
    assert!(serde_json::from_value::<workspaces::State>(short).is_err());
}

#[test]
fn wallpaper_library_payload_round_trip() {
    let payload = json!([
        {
            "id": "dusk.png",
            "name": "dusk",
            "preview": {
                "kind": "still",
                "thumbnail": { "state": "made", "file": "/cache/rikka/wallpapers/dusk.png" },
            },
        },
        {
            "id": "rain.mp4",
            "name": "rain",
            "preview": { "kind": "motion", "file": "/pictures/rain.mp4" },
        },
        {
            "id": "dawn.png",
            "name": "dawn",
            "preview": { "kind": "still", "thumbnail": { "state": "making" } },
        },
        {
            "id": "torn.png",
            "name": "torn",
            "preview": { "kind": "still", "thumbnail": { "state": "none" } },
        },
    ]);

    let listed: Vec<WallpaperEntry> =
        serde_json::from_value(payload).expect("payload should match the session shape");
    let previews: Vec<&Preview> = listed.iter().map(|listed| &listed.preview).collect();
    assert_eq!(listed[0].id, "dusk.png");
    assert_eq!(
        previews,
        [
            &Preview::Still {
                thumbnail: Thumbnail::Made {
                    file: PathBuf::from("/cache/rikka/wallpapers/dusk.png"),
                },
            },
            &Preview::Motion {
                file: PathBuf::from("/pictures/rain.mp4"),
            },
            &Preview::Still {
                thumbnail: Thumbnail::Making,
            },
            &Preview::Still {
                thumbnail: Thumbnail::None,
            },
        ]
    );
}

#[test]
fn requests_serialize_only_given_fields() {
    let show_payload = serde_json::to_value(Show {
        output: Some("DP-1".to_owned()),
        number: 7,
    })
    .expect("request should parse");
    assert_eq!(show_payload, json!({ "output": "DP-1", "number": 7 }));

    assert_eq!(
        serde_json::to_value(ActivationRequest::default()).expect("request should serialize"),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(ActivationRequest::new("rikka-files"))
            .expect("request should serialize"),
        json!({ "app_id": "rikka-files" })
    );
    let minted: ActivationToken = serde_json::from_value(json!({ "token": "rikka-7" }))
        .expect("payload should match the session shape");
    assert_eq!(minted.token, "rikka-7");
}
