use std::collections::HashMap;
use std::time::{Duration, Instant};

use discord_rate_limit::{Headers, Limiter, Route, Scope, Throttle};

fn window(remaining: u32, reset_after: u64) -> Headers {
    Headers {
        bucket: None,
        remaining: Some(remaining),
        reset_after: Some(Duration::from_secs(reset_after)),
        scope: Scope::Route,
    }
}

#[test]
fn routes_template_ids_and_keep_major_parameters() {
    let cases = [
        (
            "GET",
            "/channels/123/messages?limit=50",
            "GET /channels/:major/messages",
            "123",
        ),
        (
            "GET",
            "/channels/123/messages/456",
            "GET /channels/:major/messages/:id",
            "123",
        ),
        (
            "PATCH",
            "/guilds/7/members/8",
            "PATCH /guilds/:major/members/:id",
            "7",
        ),
        (
            "POST",
            "/webhooks/9/token",
            "POST /webhooks/:major/token",
            "9",
        ),
        ("GET", "/users/@me", "GET /users/@me", ""),
        ("GET", "/users/5/profile", "GET /users/:id/profile", ""),
        (
            "PUT",
            "/channels/1/messages/2/reactions/%F0%9F%91%8D/@me",
            "PUT /channels/:major/messages/:id/reactions/*",
            "1",
        ),
    ];
    for (method, path, template, major) in cases {
        let route = Route::new(method, path);
        assert_eq!(route.template(), template, "{path}");
        assert_eq!(route.major(), major, "{path}");
    }
}

#[test]
fn exhausted_window_waits_until_reset() {
    let route = Route::new("POST", "/channels/1/messages");
    let mut limiter = Limiter::default();
    let start = Instant::now();
    assert_eq!(limiter.take(&route, start), None);
    limiter.record(&route, &window(1, 2), start);
    assert_eq!(limiter.take(&route, start), None);
    assert_eq!(limiter.take(&route, start), Some(Duration::from_secs(2)));
    let later = start + Duration::from_secs(1);
    assert_eq!(limiter.take(&route, later), Some(Duration::from_secs(1)));
    assert_eq!(limiter.take(&route, start + Duration::from_secs(2)), None);
}

#[test]
fn shared_bucket_limits_every_route_of_one_major() {
    let send = Route::new("POST", "/channels/1/messages");
    let edit = Route::new("PATCH", "/channels/1/messages/5");
    let other = Route::new("PATCH", "/channels/2/messages/5");
    let mut limiter = Limiter::default();
    let now = Instant::now();
    let shared = |remaining| Headers {
        bucket: Some("abcd".to_owned()),
        ..window(remaining, 5)
    };
    limiter.record(&send, &shared(3), now);
    limiter.record(&edit, &shared(0), now);
    assert_eq!(limiter.take(&send, now), Some(Duration::from_secs(5)));
    assert_eq!(limiter.take(&other, now), None);
}

#[test]
fn global_throttle_holds_every_route() {
    let route = Route::new("GET", "/users/@me");
    let other = Route::new("GET", "/channels/3/messages");
    let mut limiter = Limiter::default();
    let now = Instant::now();
    limiter.throttle(&route, Duration::from_secs(3), Scope::Global, now);
    assert_eq!(limiter.take(&other, now), Some(Duration::from_secs(3)));
    limiter.throttle(&other, Duration::from_secs(1), Scope::Route, now);
    assert_eq!(limiter.take(&route, now), Some(Duration::from_secs(3)));
    let later = now + Duration::from_secs(3);
    assert_eq!(limiter.take(&route, later), None);
}

#[test]
fn responses_read_into_headers_and_throttles() {
    let headers: HashMap<&str, &str> = [
        ("x-ratelimit-bucket", "abcd"),
        ("x-ratelimit-remaining", "4"),
        ("x-ratelimit-reset-after", "1.5"),
    ]
    .into();
    let read = Headers::read(|name| headers.get(name).copied());
    assert_eq!(read.bucket.as_deref(), Some("abcd"));
    assert_eq!(read.remaining, Some(4));
    assert_eq!(read.reset_after, Some(Duration::from_millis(1500)));
    assert_eq!(read.scope, Scope::Route);

    let cases: [(Scope, &[u8], Throttle); 4] = [
        (
            Scope::Route,
            br#"{"retry_after": 0.25, "global": false}"#,
            Throttle {
                retry_after: Duration::from_millis(250),
                scope: Scope::Route,
            },
        ),
        (
            Scope::Route,
            br#"{"retry_after": 2, "global": true}"#,
            Throttle {
                retry_after: Duration::from_secs(2),
                scope: Scope::Global,
            },
        ),
        (
            Scope::Global,
            br#"{"retry_after": -1}"#,
            Throttle {
                retry_after: Duration::ZERO,
                scope: Scope::Global,
            },
        ),
        (
            Scope::Route,
            b"<html>",
            Throttle {
                retry_after: Duration::from_secs(1),
                scope: Scope::Route,
            },
        ),
    ];
    for (scope, body, expected) in cases {
        let headers = Headers {
            scope,
            ..Headers::default()
        };
        assert_eq!(Throttle::read(&headers, body), expected);
    }
}
