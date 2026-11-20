#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum NoTokens {}

#[must_use]
pub fn account_label(email: &str, detail: &str) -> String {
    match (email.is_empty(), detail.is_empty()) {
        (true, _) => detail.to_owned(),
        (false, true) => email.to_owned(),
        (false, false) => format!("{email} · {detail}"),
    }
}
