use browser_model::{DownloadPhase, Downloads, ProfileKey, download_name};
use rikka_web::DownloadItem;
use serde_json::json;

fn item(id: i64, state: &str, received: f64, error: Option<&str>) -> DownloadItem {
    serde_json::from_value(json!({
        "id": id,
        "url": format!("https://files.example/{id}.bin"),
        "filename": format!("/downloads/{id}.bin"),
        "state": state,
        "error": error,
        "bytesReceived": received,
        "totalBytes": 100.0,
    }))
    .expect("download item")
}

type Reported = Vec<(i64, DownloadPhase)>;

fn ended(downloads: &mut Downloads, snapshot: Vec<DownloadItem>) -> Reported {
    downloads
        .update(ProfileKey::Private, snapshot)
        .into_iter()
        .map(|ended| {
            let id: i64 = download_name(&ended.item)
                .trim_end_matches(".bin")
                .parse()
                .expect("numbered name");
            (id, ended.phase)
        })
        .collect()
}

#[test]
fn consecutive_snapshots_report_newly_finished_and_failed_downloads() {
    let baseline = || {
        vec![
            item(5, "complete", 100.0, None),
            item(4, "in_progress", 10.0, None),
            item(3, "in_progress", 10.0, None),
            item(2, "in_progress", 10.0, None),
        ]
    };
    let cases: [(&str, Vec<DownloadItem>, Reported); 5] = [
        (
            "finished and failed downloads are reported, cancelled ones are not",
            vec![
                item(5, "complete", 100.0, None),
                item(4, "complete", 100.0, None),
                item(3, "interrupted", 50.0, Some("NETWORK_FAILED")),
                item(2, "interrupted", 50.0, Some("USER_CANCELED")),
            ],
            vec![(4, DownloadPhase::Finished), (3, DownloadPhase::Failed)],
        ),
        (
            "a download that started and ended between snapshots is reported",
            vec![item(6, "complete", 100.0, None)]
                .into_iter()
                .chain(baseline())
                .collect(),
            vec![(6, DownloadPhase::Finished)],
        ),
        (
            "an older download coming back into the list is not reported",
            vec![
                item(4, "in_progress", 20.0, None),
                item(1, "complete", 100.0, None),
            ],
            Vec::new(),
        ),
        ("an unchanged list reports nothing", baseline(), Vec::new()),
        ("an empty list reports nothing", Vec::new(), Vec::new()),
    ];
    for (name, snapshot, expected) in cases {
        let mut downloads = Downloads::default();
        assert!(
            ended(&mut downloads, baseline()).is_empty(),
            "{name}: baseline"
        );
        assert_eq!(ended(&mut downloads, snapshot), expected, "{name}");
    }
}

#[test]
fn activity_sums_running_downloads_of_one_profile() {
    let mut downloads = Downloads::default();
    downloads.update(
        ProfileKey::Private,
        vec![
            item(3, "in_progress", 30.0, None),
            item(2, "in_progress", 70.0, None),
            item(1, "complete", 100.0, None),
        ],
    );

    let activity = downloads.activity(ProfileKey::Private);
    assert_eq!(activity.running, 2);
    assert_eq!(activity.fraction, Some(0.5));
    assert_eq!(downloads.items(ProfileKey::Private).len(), 3);
    assert!(
        downloads
            .items(ProfileKey::Persistent(browser_model::ProfileId::from_raw(
                1
            )))
            .is_empty()
    );
}
