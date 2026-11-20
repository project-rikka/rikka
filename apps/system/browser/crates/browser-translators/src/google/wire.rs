use browser_translate::{Call, Translated};
use serde_json::{Value, json};

pub(crate) fn body(call: &Call<'_>) -> Vec<u8> {
    json!([[call.strings, "auto", call.to], "te"])
        .to_string()
        .into_bytes()
}

pub(crate) fn parse(body: &[u8]) -> Option<Vec<Translated>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let texts = value.get(0)?.as_array()?;
    let detected = value.get(1).and_then(Value::as_array);
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            Some(Translated {
                text: text.as_str()?.to_owned(),
                detected: detected
                    .and_then(|detected| detected.get(index))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}
