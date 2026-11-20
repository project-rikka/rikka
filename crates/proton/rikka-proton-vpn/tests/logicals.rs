use jiff::Timestamp;
use rikka_proton_vpn::{ServerList, VpnClient};
use serde_json::json;

use rikka_test_proton::{Record, Request, Response, api, not_found, response};

const LAST_MODIFIED: &str = "Wed, 21 Oct 2015 07:28:00 GMT";

#[tokio::test(flavor = "multi_thread")]
async fn logicals_honour_if_modified_since() {
    let record = Record::default();
    let noted = record.clone();
    let api = api(move |request: Request| {
        noted.push(&request);
        match (request.route(), request.header("if-modified-since")) {
            (("GET", "/vpn/v2/logicals"), Some(_)) => response(304, Vec::new()),
            (("GET", "/vpn/v2/logicals"), None) => Response::new(
                200,
                json!({
                    "Code": 1000,
                    "StatusID": "status-one",
                    "LogicalServers": [{
                        "ID": "one",
                        "Name": "CH#1",
                        "EntryCountry": "CH",
                        "ExitCountry": "CH",
                        "Tier": 2
                    }]
                })
                .to_string(),
            )
            .header("Last-Modified", LAST_MODIFIED)
            .into_future(),
            _ => not_found(),
        }
    })
    .await;
    let client = VpnClient::new(api);

    let ServerList::Changed {
        logicals,
        last_modified,
    } = client.logicals().send().await.unwrap()
    else {
        panic!("expected changed server list");
    };
    assert_eq!(logicals.status_id.as_deref(), Some("status-one"));
    assert_eq!(logicals.servers[0].name, "CH#1");
    let last_modified = last_modified.unwrap();
    assert_eq!(
        last_modified,
        "2015-10-21T07:28:00Z".parse::<Timestamp>().unwrap()
    );

    let again = client
        .logicals()
        .modified_since(last_modified)
        .send()
        .await
        .unwrap();
    assert!(matches!(again, ServerList::Unchanged));

    let request = record.matching("GET", "/vpn/v2/logicals");
    assert_eq!(request[1].header("if-modified-since"), Some(LAST_MODIFIED));
    assert!(request[1].query.contains("WithState=true"));
}
