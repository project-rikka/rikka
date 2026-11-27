use std::sync::atomic::Ordering;

use google_account::{Grants, Profile, Service};

use crate::support::Google;

#[tokio::test]
async fn token_is_minted_once_per_service() {
    let google = Google::start();
    let grants = Grants::new(google.endpoint(), google.master().await);

    let first = grants.token(Service::Drive).await.expect("a token");
    let second = grants.token(Service::Drive).await.expect("a token");
    grants.token(Service::Mail).await.expect("a token");

    assert_eq!(first, second);
    assert_eq!(google.minted.load(Ordering::Acquire), 2);
    let minting = &google.server.requests()[1].form();
    assert_eq!(minting["app"], "com.google.android.apps.docs");
    assert_eq!(
        minting["service"],
        "oauth2:https://www.googleapis.com/auth/drive"
    );
}

#[tokio::test]
async fn unauthorized_answer_renews_the_token_once() {
    let google = Google::start();
    let grants = Grants::new(google.endpoint(), google.master().await);
    let http = rikka_net::Client::new();

    let profile = Profile::fetch_from(&http, &grants, &google.server.url("/userinfo"))
        .await
        .expect("a profile");

    assert_eq!(profile.id, "1234");
    assert_eq!(profile.email, "rikka@example.com");
    assert_eq!(google.minted.load(Ordering::Acquire), 2);
}
