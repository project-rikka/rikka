use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Claims(Value);

impl Claims {
    #[must_use]
    pub fn decode(jwt: &str) -> Option<Claims> {
        let payload = jwt.split('.').nth(1)?;
        let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
        serde_json::from_slice(&decoded).ok().map(Claims)
    }

    #[must_use]
    pub fn text(&self, path: &[&str]) -> &str {
        path.iter()
            .try_fold(&self.0, |value, step| value.get(step))
            .and_then(Value::as_str)
            .unwrap_or_default()
    }
}
