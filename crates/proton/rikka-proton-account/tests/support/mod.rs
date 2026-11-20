pub(crate) mod fixture;

use rikka_proton_account::Account;
use rikka_test_proton::{Request, ResponseFuture, api, response};

pub(crate) fn unauth_session() -> ResponseFuture {
    response(
        200,
        r#"{"Code":1000,"UID":"unauth-session","AccessToken":"unauth-access","RefreshToken":"unauth-refresh","TokenType":"Bearer","Scopes":[]}"#,
    )
}

pub(crate) async fn account<H>(handler: H) -> Account
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    Account::new(api(handler).await)
}
