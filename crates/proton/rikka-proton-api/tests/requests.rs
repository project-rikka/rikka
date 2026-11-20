use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rikka_proton_api::{
    Api, Environment, Error, MemorySessionStore, Page, Paging, Session, SessionStore,
};
use rikka_test_proton::{Record, Request, Response, ResponseFuture, response, serve};
use serde::Deserialize;

const RENEWED: &str = r#"{
    "Code": 1000,
    "AccessToken": "fresh",
    "ExpiresIn": 86400,
    "TokenType": "Bearer",
    "Scope": "self user",
    "Scopes": ["self", "user"],
    "RefreshToken": "refresh-two"
}"#;

#[derive(Debug, Deserialize)]
struct UserResponse {
    #[serde(rename = "User")]
    user: String,
}

fn api(address: &str, store: MemorySessionStore) -> Api {
    Api::builder()
        .app_version("linux-pass@1.0.0")
        .environment(Environment::custom(address).unwrap())
        .session_store(store)
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_401s_refresh_once() {
    let callers = 4;
    let arriving = Arc::new(tokio::sync::Barrier::new(callers));
    let renewals = Arc::new(AtomicUsize::new(0));
    let counted = renewals.clone();

    let address = serve(move |request: Request| {
        let arriving = arriving.clone();
        let renewals = renewals.clone();

        Box::pin(async move {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/core/v4/users") => match request.header("authorization") {
                    Some("Bearer fresh") => Response::new(200, r#"{"Code":1000,"User":"me"}"#),
                    _ => {
                        arriving.wait().await;
                        Response::new(401, r#"{"Code":401,"Error":"Invalid access token"}"#)
                    }
                },
                ("POST", "/auth/v4/refresh") => {
                    renewals.fetch_add(1, Ordering::SeqCst);
                    Response::new(200, RENEWED)
                }
                _ => Response::new(404, r#"{"Code":2501,"Error":"Not found"}"#),
            }
        }) as ResponseFuture
    })
    .await;

    let api = api(&address, MemorySessionStore::new());
    api.set_session(Session::new("uid", "stale", "refresh-one"))
        .unwrap();

    let asking: Vec<_> = (0..callers)
        .map(|_| {
            let api = api.clone();
            tokio::spawn(async move { api.get("core/v4/users").send::<UserResponse>().await })
        })
        .collect();

    for request in asking {
        assert_eq!(request.await.unwrap().unwrap().user, "me");
    }

    assert_eq!(counted.load(Ordering::SeqCst), 1);
    assert_eq!(api.session().unwrap().access_token(), "fresh");
    assert_eq!(api.session().unwrap().scopes(), ["self", "user"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_refresh_clears_session() {
    let address = serve(|request: Request| {
        Response::into_future(match request.path.as_str() {
            "/auth/v4/refresh" => {
                Response::new(422, r#"{"Code":10013,"Error":"Invalid refresh token"}"#)
            }
            _ => Response::new(401, r#"{"Code":401,"Error":"Invalid access token"}"#),
        })
    })
    .await;

    let store = Arc::new(MemorySessionStore::with_session(Session::new(
        "uid", "stale", "gone",
    )));
    let api = Api::builder()
        .app_version("linux-pass@1.0.0")
        .environment(Environment::custom(&address).unwrap())
        .session_store(store.clone())
        .build()
        .unwrap();

    let request = api.get("core/v4/users").send::<UserResponse>().await;

    assert!(matches!(request, Err(Error::SessionExpired)));
    assert!(api.session().is_none());
    assert!(store.load().unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn anonymous_request_skips_session() {
    let renewals = Arc::new(AtomicUsize::new(0));
    let counted = renewals.clone();

    let address = serve(move |request: Request| {
        let renewals = renewals.clone();
        Response::into_future(match request.path.as_str() {
            "/auth/v4/refresh" => {
                renewals.fetch_add(1, Ordering::SeqCst);
                Response::new(200, RENEWED)
            }
            _ => match request.header("x-pm-uid") {
                Some(_) => Response::new(200, r#"{"Code":1000,"User":"session"}"#),
                None => Response::new(401, r#"{"Code":401,"Error":"Invalid access token"}"#),
            },
        })
    })
    .await;

    let api = api(&address, MemorySessionStore::new());
    api.set_session(Session::new("uid", "stale", "refresh-one"))
        .unwrap();

    let request = api
        .get("core/v4/users")
        .anonymous()
        .send::<UserResponse>()
        .await;

    assert!(matches!(request, Err(Error::Unauthorized)));
    assert_eq!(counted.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn rate_limit_honours_retry_after() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();

    let address = serve(move |_: Request| {
        let attempts = attempts.clone();
        Response::into_future(match attempts.fetch_add(1, Ordering::SeqCst) {
            0 => Response::new(429, r#"{"Code":2028,"Error":"Too many requests"}"#)
                .header("Retry-After", "0"),
            _ => Response::new(200, r#"{"Code":1000,"User":"me"}"#),
        })
    })
    .await;

    let api = api(&address, MemorySessionStore::new());
    let request = api
        .get("core/v4/users")
        .send::<UserResponse>()
        .await
        .unwrap();

    assert_eq!(request.user, "me");
    assert_eq!(counted.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn error_code_fails_successful_status() {
    let address =
        serve(|_: Request| response(200, r#"{"Code":2001,"Error":"Invalid value"}"#)).await;

    let api = api(&address, MemorySessionStore::new());

    let Err(Error::Api(refusal)) = api.get("core/v4/users").send::<UserResponse>().await else {
        panic!("error code accepted as success");
    };
    assert_eq!(refusal.code, rikka_proton_api::ResponseCode::InvalidValue);
    assert_eq!(refusal.message, "Invalid value");

    let bytes = api.get("core/v4/blocks").send_bytes().await.unwrap();
    assert_eq!(bytes, r#"{"Code":2001,"Error":"Invalid value"}"#);
}

#[tokio::test(flavor = "multi_thread")]
async fn sibling_apis_keep_separate_sessions() {
    let address = serve(|request: Request| {
        Response::into_future(match request.header("authorization") {
            Some("Bearer alice-token") => Response::new(200, r#"{"Code":1000,"User":"alice"}"#),
            Some("Bearer bob-token") => Response::new(200, r#"{"Code":1000,"User":"bob"}"#),
            _ => Response::new(401, r#"{"Code":401,"Error":"Invalid access token"}"#),
        })
    })
    .await;
    let base = api(&address, MemorySessionStore::new());
    let alice_store = Arc::new(MemorySessionStore::with_session(Session::new(
        "alice",
        "alice-token",
        "alice-refresh",
    )));
    let alice = base.with_session_store(alice_store.clone()).unwrap();
    let bob_store = Arc::new(MemorySessionStore::new());
    let bob = base.with_session_store(bob_store.clone()).unwrap();
    bob.set_session(Session::new("bob", "bob-token", "bob-refresh"))
        .unwrap();

    assert_eq!(
        alice
            .get("core/v4/users")
            .send::<UserResponse>()
            .await
            .unwrap()
            .user,
        "alice"
    );
    assert_eq!(
        bob.get("core/v4/users")
            .send::<UserResponse>()
            .await
            .unwrap()
            .user,
        "bob"
    );
    assert!(base.session().is_none());
    assert_eq!(bob_store.load().unwrap().unwrap().uid(), "bob");
    assert_eq!(alice_store.load().unwrap().unwrap().uid(), "alice");
}

#[tokio::test(flavor = "multi_thread")]
async fn absolute_url_gets_no_credentials() {
    let seen = Record::default();
    let record = seen.clone();
    let storage = serve(move |request: Request| {
        record.push(&request);
        Response::into_future(Response::new(401, "denied"))
    })
    .await;

    let api = api("https://api.invalid/", MemorySessionStore::new());
    api.set_session(Session::new("uid", "access", "refresh-one"))
        .unwrap();

    let stored = api
        .post(format!("{storage}/storage/block"))
        .send_bytes()
        .await;

    assert!(matches!(stored, Err(Error::Unauthorized)));
    let requests = seen.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].header("authorization"), None);
    assert_eq!(requests[0].header("x-pm-uid"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn field_is_read_out_of_the_envelope() {
    let address = serve(|_: Request| {
        Response::into_future(Response::new(
            200,
            r#"{"Code":1000,"Ignored":{"Deep":[1,2]},"User":"me","Total":null,"Folder":{"Name":"x","ID":"f1"},"Empty":null}"#,
        ))
    })
    .await;
    let api = api(&address, MemorySessionStore::new());

    let user: String = api.get("core/v4/users").send_field("User").await.unwrap();
    let absent: Option<String> = api.get("core/v4/users").send_field("Absent").await.unwrap();
    let defaulted: Vec<String> = api
        .get("core/v4/users")
        .send_field_or_default("Absent")
        .await
        .unwrap();
    let missing = api
        .get("core/v4/users")
        .send_field::<String>("Absent")
        .await;
    let nested: String = api
        .get("core/v4/users")
        .send_field_at(&["Folder", "ID"])
        .await
        .unwrap();
    let under_null: Option<String> = api
        .get("core/v4/users")
        .send_field_at(&["Empty", "ID"])
        .await
        .unwrap();

    assert_eq!(user, "me");
    assert_eq!(absent, None);
    assert!(defaulted.is_empty());
    assert!(matches!(missing, Err(Error::Decode { .. })));
    assert_eq!(nested, "f1");
    assert_eq!(under_null, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn paged_field_walks_until_a_page_lacks_it() {
    let address = serve(|request: Request| {
        let body = match request.query("Since") {
            None => r#"{"Code":1000,"Page":{"Items":[1,2],"LastID":"b"}}"#,
            Some("b") => r#"{"Code":1000,"Page":{"Items":[3],"LastID":"c"}}"#,
            Some(_) => r#"{"Code":1000}"#,
        };
        Response::into_future(Response::new(200, body))
    })
    .await;
    let api = api(&address, MemorySessionStore::new());

    #[derive(Deserialize)]
    struct Listed {
        #[serde(rename = "Items")]
        items: Vec<u32>,
        #[serde(rename = "LastID")]
        last: String,
    }
    let items = api
        .get("items")
        .pages(Paging::since())
        .field("Page")
        .collect(|page: Listed| Page::new(page.items).next(Some(page.last)))
        .await
        .unwrap();

    assert_eq!(items, [1, 2, 3]);
}
