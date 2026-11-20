use rikka_proton_vpn::{BinaryStatus, Error, Scoring, ServerLocation, VpnClient};
use serde_json::json;

use crate::support::logical;
use rikka_test_proton::{Request, api, binary_response, not_found};

const TWO_SERVERS: &str = "AQAAAAMyAAAAAAJLAACAPw==";

fn bytes(encoded: &str) -> Vec<u8> {
    rikka_proton_api::base64::decode(encoded).unwrap()
}

fn record(flags: u8, load: u8, partial_score: f32) -> Vec<u8> {
    let mut record = vec![flags, load];
    record.extend_from_slice(&partial_score.to_le_bytes());
    record
}

fn file(records: &[Vec<u8>]) -> Vec<u8> {
    let mut file = vec![1, 0, 0, 0];
    for record in records {
        file.extend_from_slice(record);
    }
    file
}

#[test]
fn binary_status_parses_and_rejects_corrupt() {
    let status = BinaryStatus::parse(bytes(TWO_SERVERS)).unwrap();
    assert_eq!(status.len(), 2);

    let first = status.get(0).unwrap();
    assert!(first.is_enabled() && first.is_visible() && !first.is_autoconnectable());
    assert_eq!((first.load(), first.partial_score()), (50, 0.0));
    let second = status.get(1).unwrap();
    assert!(!second.is_enabled() && second.is_visible());
    assert_eq!((second.load(), second.partial_score()), (75, 1.0));
    assert!(status.get(2).is_none());

    let unlikely = BinaryStatus::parse(file(&[record(3, 150, 0.5), record(3, 10, 7.0)])).unwrap();
    assert!(unlikely.get(0).is_none());
    assert!(unlikely.get(1).is_none());

    for damaged in [vec![8_u8], vec![8, 7, 6, 5], vec![1, 0, 0, 0, 0]] {
        assert!(matches!(
            BinaryStatus::parse(&damaged),
            Err(Error::MalformedStatus { .. })
        ));
    }
}

#[test]
fn scoring_matches_official_clients() {
    let status = BinaryStatus::parse(file(&[
        record(3, 50, 0.0),
        record(2, 75, 1.0),
        record(3, 20, 0.5),
    ]))
    .unwrap();
    let paris = json!({"Latitude": 48.8566, "Longitude": 2.3522});
    let annecy = json!({"Latitude": 45.8992, "Longitude": 6.1294});
    let logical_entry = |id: &str, index: u32, country: &str, location: &serde_json::Value| {
        logical(json!({
            "ID": id,
            "ExitCountry": country,
            "Load": 10,
            "StatusReference": {"Index": index, "Penalty": 0.0, "Cost": 0},
            "EntryLocation": location,
            "ExitLocation": location
        }))
    };
    let mut servers = vec![
        logical_entry("enabled", 0, "FR", &paris),
        logical_entry("disabled", 1, "FR", &paris),
        logical_entry("annecy", 2, "FR", &annecy),
        logical_entry("absent", 9, "FR", &paris),
        logical(json!({"ID": "unreferenced", "Load": 30, "Score": 7.0})),
    ];

    Scoring::new()
        .with_location(ServerLocation {
            latitude: 46.2044,
            longitude: 6.1432,
        })
        .with_country("ch")
        .without_jitter()
        .apply(&status, &mut servers);

    let enabled = &servers[0];
    assert_eq!(
        (enabled.load, enabled.status, enabled.visible),
        (Some(50), Some(1), true)
    );
    let disabled = &servers[1];
    assert_eq!((disabled.load, disabled.status), (Some(75), Some(0)));
    assert!(disabled.score.unwrap() > 1_000.0);
    assert!((servers[2].score.unwrap() - 1.5).abs() < 1e-9);
    let absent = &servers[3];
    assert_eq!((absent.status, absent.visible), (Some(0), false));
    let unreferenced = &servers[4];
    assert_eq!(
        (unreferenced.load, unreferenced.score),
        (Some(30), Some(7.0))
    );

    let mut near = vec![logical_entry("paris", 0, "FR", &paris)];
    Scoring::new()
        .with_location(ServerLocation {
            latitude: 43.6047,
            longitude: 1.4442,
        })
        .with_country("FR")
        .without_jitter()
        .apply(&status, &mut near);
    let score = near[0].score.unwrap();
    assert!((0.87..0.88).contains(&score), "{score}");
}

#[tokio::test(flavor = "multi_thread")]
async fn binary_status_fetched_by_id() {
    let api = api(|request: Request| match request.route() {
        ("GET", "/vpn/v2/status/status-one/binary") => binary_response(200, bytes(TWO_SERVERS)),
        _ => not_found(),
    })
    .await;

    let status = VpnClient::new(api).status("status-one").await.unwrap();
    assert_eq!(status.len(), 2);
    assert_eq!(status.get(1).unwrap().load(), 75);
}
