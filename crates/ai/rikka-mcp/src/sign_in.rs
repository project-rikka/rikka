use rikka_oauth::{Loopback, Registration};

use crate::authorization::Authority;
use crate::credentials::Registrant;
use crate::http::Http;
use crate::oauth::Oauth;
use crate::vault::Vault;

const CALLBACK: &str = "/callback";
const CLIENT: &str = "Rikka";

pub(crate) async fn sign_in(
    http: &Http,
    vault: &Vault,
    challenge: Option<&str>,
) -> Result<(), String> {
    let declared = http.oauth();
    let port = declared.and_then(Oauth::callback_port).unwrap_or(0);
    let redirect = Loopback::localhost(port)
        .path(CALLBACK)
        .bind()
        .await
        .map_err(|error| format!("no page could wait for the answer: {error}"))?;

    let authority = Authority::discover(http.url(), challenge).await?;
    let (client_id, client_secret) = match declared.and_then(Oauth::client_id) {
        Some(id) => (
            id.to_owned(),
            declared.and_then(Oauth::client_secret).map(str::to_owned),
        ),
        None => {
            let endpoint = authority.registration_endpoint().ok_or(
                "the sign-in offered registers no new clients; declare an oauth client id",
            )?;
            let mut registration = Registration::new(CLIENT).redirect_uri(redirect.uri());
            if let Some(scope) = authority.scope() {
                registration = registration.scope(scope);
            }
            let registered = registration
                .send(endpoint.as_str())
                .await
                .map_err(|error| format!("this client could not be registered: {error}"))?;
            (
                registered.client_id().to_owned(),
                registered.client_secret().map(str::to_owned),
            )
        }
    };
    let registrant = Registrant::new(
        client_id,
        client_secret,
        authority.supports_secret_basic(),
        authority.token_endpoint().to_string(),
        authority.resource().to_owned(),
    );
    let client = registrant.client().map_err(|error| error.to_string())?;

    let mut request = client
        .authorization(authority.authorization_endpoint().as_str(), redirect.uri())
        .pkce()
        .param("resource", authority.resource());
    if let Some(scope) = authority.scope() {
        request = request.scope(scope);
    }
    let authorization = request.build().map_err(|error| error.to_string())?;
    rikka_open::open(authorization.url())
        .map_err(|error| format!("the sign-in page did not open: {error}"))?;

    let callback = redirect
        .callback(authorization.state())
        .await
        .map_err(|error| error.to_string())?;
    let code = callback.code().map_err(|error| error.to_string())?;
    authority
        .check_response_issuer(callback.get("iss"))
        .map_err(|error| error.to_string())?;
    let token = client
        .exchange(&code, &authorization)
        .field("resource", authority.resource())
        .send()
        .await
        .map_err(|error| error.to_string())?;

    vault
        .entry(http.url())
        .save(&registrant.granted(token))
        .await
        .map_err(|error| format!("the sign-in could not be kept: {error}"))
}
