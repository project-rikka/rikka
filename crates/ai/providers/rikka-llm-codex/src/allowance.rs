use serde_json::Value;

use rikka_llm::wire::{self, text_of};
use rikka_llm::{Scope, Span, Window};

pub(super) fn read(body: &Value) -> Vec<Window> {
    let mut windows = metered(body.get("rate_limit"), &Scope::Everything);
    windows.extend(features(body));
    windows.extend(ceiling(body));
    windows
}

fn metered(published: Option<&Value>, scope: &Scope) -> Vec<Window> {
    let Some(published) = published else {
        return Vec::new();
    };

    ["primary_window", "secondary_window"]
        .into_iter()
        .filter_map(|field| window(published.get(field)?, scope.clone()))
        .collect()
}

fn window(published: &Value, scope: Scope) -> Option<Window> {
    let used = published.get("used_percent")?.as_f64()?;
    let span = seconds(published, "limit_window_seconds").map_or(Span::Unnamed, wire::span_of);
    Some(wire::window(
        scope,
        span,
        used,
        seconds(published, "reset_at"),
    ))
}

fn features(body: &Value) -> Vec<Window> {
    let Some(published) = body.get("additional_rate_limits").and_then(Value::as_array) else {
        return Vec::new();
    };

    published
        .iter()
        .flat_map(|feature| match named(feature) {
            Some(name) => metered(feature.get("rate_limit"), &Scope::Part(name)),
            None => Vec::new(),
        })
        .collect()
}

fn named(feature: &Value) -> Option<String> {
    let name = match text_of(feature, "limit_name") {
        empty if empty.is_empty() => text_of(feature, "metered_feature"),
        name => name,
    };
    Some(name).filter(|name| !name.is_empty())
}

fn ceiling(body: &Value) -> Option<Window> {
    let published = body.get("spend_control")?.get("individual_limit")?;
    let used = published.get("used_percent")?.as_f64()?;
    Some(wire::window(
        Scope::Extra,
        Span::Monthly,
        used,
        seconds(published, "reset_at"),
    ))
}

fn seconds(published: &Value, field: &str) -> Option<u64> {
    published.get(field)?.as_u64()
}
