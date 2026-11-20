pub(crate) mod agent;

use rikka_proton_vpn::LogicalServer;
use serde_json::{Value, json};

pub(crate) const PEER_KEY: &str = "hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=";

pub(crate) fn logical(fields: Value) -> LogicalServer {
    let mut server = json!({
        "ID": "server",
        "Name": "CH#1",
        "EntryCountry": "CH",
        "ExitCountry": "CH",
        "City": "Zurich",
        "Tier": 2,
        "Features": 0,
        "Score": 1.0,
        "Status": 1,
        "Servers": [{
            "ID": "domain",
            "Domain": agent::SERVER_NAME,
            "EntryIP": "185.159.157.1",
            "Status": 1,
            "X25519PublicKey": PEER_KEY
        }]
    });
    if let (Some(target), Some(fields)) = (server.as_object_mut(), fields.as_object()) {
        for (name, value) in fields {
            target.insert(name.clone(), value.clone());
        }
    }
    serde_json::from_value(server).unwrap()
}
