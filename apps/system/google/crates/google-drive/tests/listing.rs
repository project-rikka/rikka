use google_drive::{Kind, Scope};
use google_test_server::Response;
use serde_json::json;

use crate::support::{FOLDER, Google, file};

#[tokio::test]
async fn folder_listing_follows_every_page() {
    let google = Google::start(|request| {
        let query = request.query();
        match query.get("pageToken").map(String::as_str) {
            None => Response::json(
                &json!({
                    "nextPageToken": "second",
                    "files": [file("a", "Reports", FOLDER), file("b", "photo.jpg", "image/jpeg")],
                })
                .to_string(),
            ),
            Some("second") => Response::json(
                &json!({ "files": [file("c", "notes.txt", "text/plain")] }).to_string(),
            ),
            Some(_) => Response::status(400),
        }
    });
    let drive = google.drive().await;

    let files = drive
        .list_all(&Scope::Folder("it's".to_owned()))
        .await
        .expect("a listing");

    assert_eq!(files.len(), 3);
    assert!(files[0].is_folder());
    assert_eq!(files[1].kind(), Kind::Image);
    assert_eq!(files[1].size, Some(1024));
    assert_eq!(files[1].modified_time, Some(1_700_000_000_000));
    assert!(files[1].capabilities.can_rename && !files[1].capabilities.can_trash);
    let first = &google.asked("/drive/v3/files")[0].query();
    assert_eq!(first["q"], "'it\\'s' in parents and trashed = false");
    assert_eq!(first["orderBy"], "folder,name_natural");
    assert_eq!(first["includeItemsFromAllDrives"], "true");
    assert!(first["fields"].starts_with("nextPageToken,files(id,name,mimeType"));
}

#[tokio::test]
async fn scopes_ask_their_own_queries() {
    let google = Google::start(|_| Response::json(r#"{"files":[]}"#));
    let drive = google.drive().await;
    let cases = [
        (Scope::Starred, "starred = true and trashed = false"),
        (Scope::Trash, "trashed = true"),
        (
            Scope::SharedWithMe,
            "sharedWithMe = true and trashed = false",
        ),
        (
            Scope::Search(" tax 'draft' ".to_owned()),
            "(name contains 'tax \\'draft\\'' or fullText contains 'tax \\'draft\\'') and trashed = false",
        ),
    ];

    for (scope, _) in &cases {
        drive.list(scope, None).await.expect("a listing");
    }

    let asked = google.asked("/drive/v3/files");
    for (request, (scope, filter)) in asked.iter().zip(&cases) {
        assert_eq!(&request.query()["q"], filter, "{scope:?}");
    }
    assert!(!asked[3].query().contains_key("orderBy"));
}

#[tokio::test]
async fn shortcuts_and_shared_drives_resolve_to_their_targets() {
    let google = Google::start(|request| match request.path() {
        "/drive/v3/drives" => Response::json(r#"{"drives":[{"id":"team-1","name":"Team"}]}"#),
        _ => Response::json(
            &json!({ "files": [{
                "id": "link",
                "name": "Budget",
                "mimeType": "application/vnd.google-apps.shortcut",
                "shortcutDetails": {
                    "targetId": "sheet",
                    "targetMimeType": "application/vnd.google-apps.spreadsheet",
                },
                "hasThumbnail": true,
                "thumbnailLink": "https://lh3.example/abc=s220",
            }] })
            .to_string(),
        ),
    });
    let drive = google.drive().await;

    let shortcut = drive
        .list(&Scope::my_drive(), None)
        .await
        .expect("a listing")
        .files
        .remove(0);
    let drives = drive
        .list(&Scope::SharedDrives, None)
        .await
        .expect("drives")
        .files;

    assert!(shortcut.is_shortcut() && shortcut.is_native() && !shortcut.is_folder());
    assert_eq!(shortcut.target_id(), "sheet");
    assert_eq!(shortcut.kind(), Kind::Spreadsheet);
    assert_eq!(
        shortcut.thumbnail(96).as_deref(),
        Some("https://lh3.example/abc=s96")
    );
    assert!(drives[0].is_folder());
    assert_eq!(
        (drives[0].id.as_str(), drives[0].name.as_str()),
        ("team-1", "Team")
    );
}

#[tokio::test]
async fn changes_are_read_up_to_the_new_cursor() {
    let google = Google::start(|request| match request.path() {
        "/drive/v3/changes/startPageToken" => Response::json(r#"{"startPageToken":"10"}"#),
        _ => match request.query()["pageToken"].as_str() {
            "10" => Response::json(
                &json!({
                    "nextPageToken": "11",
                    "changes": [{ "fileId": "a", "file": file("a", "a.txt", "text/plain") }],
                })
                .to_string(),
            ),
            _ => Response::json(
                r#"{"newStartPageToken":"12","changes":[{"fileId":"b","removed":true}]}"#,
            ),
        },
    });
    let drive = google.drive().await;

    let cursor = drive.start_cursor().await.expect("a cursor");
    let changes = drive.changes(&cursor).await.expect("changes");

    assert_eq!(changes.cursor, "12");
    assert_eq!(changes.changes.len(), 2);
    assert_eq!(
        changes.changes[0]
            .file
            .as_ref()
            .map(|file| file.name.as_str()),
        Some("a.txt")
    );
    assert!(changes.changes[1].file.is_none());
}
