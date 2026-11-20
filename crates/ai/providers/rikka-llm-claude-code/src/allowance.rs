use serde_json::Value;

use rikka_llm::wire;
use rikka_llm::{Scope, Span, Window};

const WHOLE: [(&str, Span); 2] = [("five_hour", Span::Session), ("seven_day", Span::Weekly)];
const NAMED: [(&str, &str); 2] = [("seven_day_opus", "Opus"), ("seven_day_sonnet", "Sonnet")];

pub(super) fn read(body: &Value) -> Vec<Window> {
    let scoped = scoped(body);
    let mut windows = Vec::new();

    for (field, span) in WHOLE {
        windows.extend(
            body.get(field)
                .and_then(|published| metered(published, Scope::Everything, span)),
        );
    }
    for (field, part) in NAMED {
        if scoped.iter().any(|window| names(window, part)) {
            continue;
        }
        windows.extend(
            body.get(field).and_then(|published| {
                metered(published, Scope::Part(part.to_owned()), Span::Weekly)
            }),
        );
    }
    windows.extend(scoped);
    windows.extend(extra(body));
    windows
}

fn metered(published: &Value, scope: Scope, span: Span) -> Option<Window> {
    let used = published.get("utilization")?.as_f64()?;
    Some(wire::window(scope, span, used, resets_at(published)))
}

fn scoped(body: &Value) -> Vec<Window> {
    let Some(rows) = body.get("limits").and_then(Value::as_array) else {
        return Vec::new();
    };

    rows.iter().filter_map(part).collect()
}

fn part(row: &Value) -> Option<Window> {
    let name = row.pointer("/scope/model/display_name")?.as_str()?;
    let used = row.get("percent")?.as_f64()?;
    Some(wire::window(
        Scope::Part(name.to_owned()),
        grouped(row),
        used,
        resets_at(row),
    ))
}

fn grouped(row: &Value) -> Span {
    match row.get("group").and_then(Value::as_str) {
        Some("session") => Span::Session,
        Some("weekly") => Span::Weekly,
        Some("monthly") => Span::Monthly,
        _ => Span::Unnamed,
    }
}

fn names(window: &Window, part: &str) -> bool {
    matches!(window.scope(), Scope::Part(name) if name.eq_ignore_ascii_case(part))
}

fn extra(body: &Value) -> Option<Window> {
    let published = body.get("extra_usage")?;
    let enabled = published
        .get("is_enabled")
        .and_then(Value::as_bool)
        .unwrap_or_default();
    let unlimited = published.get("monthly_limit")?.as_f64().is_none();
    if !enabled || unlimited {
        return None;
    }

    let used = published.get("utilization")?.as_f64()?;
    Some(wire::window(Scope::Extra, Span::Monthly, used, None))
}

fn resets_at(published: &Value) -> Option<u64> {
    let reset_time = published.get("resets_at")?.as_str()?;
    let at: jiff::Timestamp = reset_time.parse().ok()?;
    u64::try_from(at.as_second()).ok()
}
