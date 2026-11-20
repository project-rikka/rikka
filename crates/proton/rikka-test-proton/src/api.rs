use std::time::Duration;

use rikka_proton_api::{Api, ApiBuilder, Environment, Session};

use crate::request::Request;
use crate::response::ResponseFuture;
use crate::server::serve;

const APP_VERSION: &str = "linux-pass@1.0.0";
const TIMEOUT: Duration = Duration::from_secs(5);

pub fn builder(address: &str) -> ApiBuilder {
    Api::builder()
        .app_version(APP_VERSION)
        .environment(Environment::custom(address).unwrap())
        .timeout(TIMEOUT)
}

pub async fn api<H>(handler: H) -> Api
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    builder(&serve(handler).await).build().unwrap()
}

pub async fn signed_in_api<H>(handler: H) -> Api
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    builder(&serve(handler).await)
        .session(Session::new(
            "signed-in-uid",
            "access-value",
            "refresh-value",
        ))
        .build()
        .unwrap()
}
