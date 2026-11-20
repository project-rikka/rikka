use rikka_certificate_transparency::{LogList, LogListKey};
use serde_json::Value;

pub const CHROME_LIST: &[u8] = include_bytes!("../data/log_list.json");
pub const CHROME_SIGNATURE: &[u8] = include_bytes!("../data/log_list.sig");

pub fn chrome_log_list() -> LogList {
    LogList::from_signed(CHROME_LIST, CHROME_SIGNATURE, &LogListKey::chrome())
        .expect("the fixture should carry Chrome's signature")
}

pub fn log_list_at(timestamp: &str) -> Vec<u8> {
    edited(timestamp, |_| {})
}

pub fn log_list_with_states(timestamp: &str, state: &Value) -> Vec<u8> {
    edited(timestamp, |log| log["state"] = state.clone())
}

fn edited(timestamp: &str, mut edit: impl FnMut(&mut Value)) -> Vec<u8> {
    let mut document: Value =
        serde_json::from_slice(CHROME_LIST).expect("the fixture should be JSON");
    document["log_list_timestamp"] = Value::from(timestamp);
    let operators = document["operators"]
        .as_array_mut()
        .expect("the fixture should list operators");
    for operator in operators {
        for kind in ["logs", "tiled_logs"] {
            for log in operator[kind].as_array_mut().into_iter().flatten() {
                edit(log);
            }
        }
    }
    serde_json::to_vec_pretty(&document).expect("an edited fixture serializes")
}
