use serde_json::Value;

use rikka_llm::wire;
use rikka_llm::{Scope, Span, Window};

const CADENCES: [(&str, Span); 6] = [
    ("session", Span::Session),
    ("5h", Span::Session),
    ("5-hour", Span::Session),
    ("five-hour", Span::Session),
    ("five hour", Span::Session),
    ("weekly", Span::Weekly),
];
const LIMIT_SUFFIX: &str = " limit";

pub(super) fn read(body: &Value) -> Vec<Window> {
    let summary = body.get("response").unwrap_or(body);
    let Some(groups) = summary.get("groups").and_then(Value::as_array) else {
        return Vec::new();
    };

    groups.iter().flat_map(windows).collect()
}

fn windows(group: &Value) -> Vec<Window> {
    let Some(buckets) = group.get("buckets").and_then(Value::as_array) else {
        return Vec::new();
    };

    let family = display_name(group);
    buckets
        .iter()
        .filter_map(|bucket| window(family, bucket))
        .collect()
}

fn window(family: Option<&str>, bucket: &Value) -> Option<Window> {
    if bucket.get("disabled").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let remaining = bucket
        .get("remainingFraction")
        .or_else(|| bucket.pointer("/remaining/remainingFraction"))?
        .as_f64()?;

    let span = cadence(bucket);
    let scope = match (family, span, display_name(bucket)) {
        (Some(family), Span::Unnamed, Some(limit)) => Scope::Part(format!("{family} · {limit}")),
        (Some(family), _, _) => Scope::Part(family.to_owned()),
        (None, Span::Unnamed, Some(limit)) => Scope::Part(limit.to_owned()),
        (None, _, _) => Scope::Everything,
    };
    Some(wire::window(
        scope,
        span,
        (1.0 - remaining) * 100.0,
        resets_at(bucket),
    ))
}

fn cadence(bucket: &Value) -> Span {
    let named = match bucket.get("window").and_then(Value::as_str).map(str::trim) {
        Some(window) if !window.is_empty() => vec![window],
        _ => ["bucketId", "displayName"]
            .into_iter()
            .filter_map(|field| bucket.get(field).and_then(Value::as_str))
            .collect(),
    };

    named
        .into_iter()
        .find_map(|name| span_of(&normalized(name)))
        .unwrap_or(Span::Unnamed)
}

fn normalized(name: &str) -> String {
    let name = name.trim().to_lowercase().replace('_', "-");
    match name.strip_suffix(LIMIT_SUFFIX) {
        Some(cadence) => cadence.to_owned(),
        None => name,
    }
}

fn span_of(name: &str) -> Option<Span> {
    CADENCES
        .into_iter()
        .find(|(cadence, _)| {
            name == *cadence
                || name
                    .strip_suffix(cadence)
                    .is_some_and(|stem| stem.ends_with('-'))
        })
        .map(|(_, span)| span)
}

fn display_name(value: &Value) -> Option<&str> {
    value
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn resets_at(bucket: &Value) -> Option<u64> {
    let published = bucket.get("resetTime")?;
    if let Some(seconds) = published.as_u64() {
        return Some(seconds);
    }
    let at: jiff::Timestamp = published.as_str()?.parse().ok()?;
    u64::try_from(at.as_second()).ok()
}
