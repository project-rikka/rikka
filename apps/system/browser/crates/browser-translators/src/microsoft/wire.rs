use browser_translate::{Call, Translated};
use serde_json::{Value, json};

pub(crate) fn body(call: &Call<'_>) -> Vec<u8> {
    json!(call.strings).to_string().into_bytes()
}

pub(crate) fn parse(body: &[u8]) -> Option<Vec<Translated>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    value
        .as_array()?
        .iter()
        .map(|result| {
            Some(Translated {
                text: result
                    .get("translations")?
                    .get(0)?
                    .get("text")?
                    .as_str()?
                    .to_owned(),
                detected: result
                    .get("detectedLanguage")
                    .and_then(|detected| detected.get("language"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}
