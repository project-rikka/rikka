use rikka_proton_vpn::{LogicalServer, ServerFeatures, ServerQuery, Tier};
use serde_json::{Value, json};

use crate::support::{PEER_KEY, logical};

fn server(id: &str, ip: &str, fields: Value) -> LogicalServer {
    let mut fields = fields;
    fields["ID"] = json!(id);
    fields["Name"] = json!(id);
    if fields.get("Servers").is_none() {
        fields["Servers"] = json!([{
            "Domain": format!("{id}.protonvpn.net"),
            "EntryIP": ip,
            "Status": 1,
            "X25519PublicKey": PEER_KEY
        }]);
    }
    logical(fields)
}

fn servers() -> Vec<LogicalServer> {
    vec![
        server("zurich", "10.0.0.1", json!({"Score": 1.0})),
        server(
            "geneva",
            "10.0.0.2",
            json!({"Score": 2.0, "Tier": 0, "City": "Geneva"}),
        ),
        server(
            "stockholm",
            "10.0.0.3",
            json!({"Score": 0.5, "ExitCountry": "SE", "Features": 1, "City": "Stockholm"}),
        ),
        server(
            "newyork",
            "10.0.0.4",
            json!({"Score": 3.0, "ExitCountry": "US", "Features": 4, "Status": 0, "City": "New York"}),
        ),
        server(
            "acme",
            "10.0.0.5",
            json!({"Score": 0.1, "Features": 32, "GatewayName": "Acme"}),
        ),
        server(
            "keyless",
            "10.0.0.6",
            json!({"Score": 0.2, "Servers": [{"Domain": "keyless.protonvpn.net", "EntryIP": "10.0.0.6", "Status": 1}]}),
        ),
        server("basel", "10.0.0.7", json!({"Score": 1.5, "City": "Basel"})),
    ]
}

#[test]
fn query_picks_lowest_score_in_tier() {
    let listed = servers();
    let cases = [
        (ServerQuery::fastest(), Tier::PLUS, Some("zurich")),
        (ServerQuery::fastest(), Tier::FREE, Some("geneva")),
        (
            ServerQuery::fastest().country("ch").city("Geneva"),
            Tier::PLUS,
            Some("geneva"),
        ),
        (
            ServerQuery::fastest().secure_core(true),
            Tier::PLUS,
            Some("stockholm"),
        ),
        (ServerQuery::gateway("Acme"), Tier::PLUS, Some("acme")),
        (
            ServerQuery::fastest().features(ServerFeatures::P2P),
            Tier::PLUS,
            Some("newyork"),
        ),
        (ServerQuery::server("geneva"), Tier::PLUS, Some("geneva")),
        (
            ServerQuery::fastest().excluding_country("CH"),
            Tier::PLUS,
            Some("newyork"),
        ),
        (ServerQuery::fastest().country("JP"), Tier::PLUS, None),
    ];

    for (index, (query, tier, expected)) in cases.into_iter().enumerate() {
        let found = query.best(&listed, tier).map(|server| server.id.as_str());
        assert_eq!(found, expected, "case {index}");
    }
}

#[test]
fn fallbacks_skip_failed_entry() {
    let listed = servers();
    let failed = &listed[0];
    let query = ServerQuery::fastest().country("CH");

    let fallbacks: Vec<&str> = query
        .fallbacks(&listed, Tier::PLUS, failed)
        .into_iter()
        .map(|server| server.id.as_str())
        .collect();

    assert!(!fallbacks.contains(&"zurich"));
    assert!(!fallbacks.contains(&"acme"));
    assert!(!fallbacks.contains(&"keyless"));
    assert!(!fallbacks.contains(&"newyork"));
    assert_eq!(fallbacks.first(), Some(&"basel"));
    assert!(fallbacks.contains(&"geneva"));
    assert!(fallbacks.len() <= 5);
}
