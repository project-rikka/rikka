use serde_json::json;

use crate::asked::Asked;
use crate::reply::Reply;

const RESOURCE_METADATA: &str = "/.well-known/oauth-protected-resource/mcp";
const ISSUER_METADATA: &str = "/.well-known/oauth-authorization-server/auth";
const SCOPE: &str = "tools";
const CODE: &str = "granted";
const SHORT_LIVED: u64 = 60;
const LONG_LIVED: u64 = 3600;

#[derive(Debug, Default)]
pub(crate) struct Guard {
    issued: Vec<String>,
}

impl Guard {
    pub(crate) fn admits(&self, asked: &Asked) -> bool {
        asked
            .header("authorization")
            .and_then(|credentials| credentials.strip_prefix("Bearer "))
            .is_some_and(|token| self.issued.iter().any(|issued| issued == token))
    }

    pub(crate) fn challenge(base: &str) -> Reply {
        Reply::status("401 Unauthorized").with_header(
            "WWW-Authenticate",
            format!("Bearer resource_metadata=\"{base}{RESOURCE_METADATA}\", scope=\"{SCOPE}\""),
        )
    }

    pub(crate) fn page(&mut self, base: &str, asked: &Asked) -> Option<Reply> {
        let issuer = format!("{base}/auth");
        match (asked.method(), asked.path()) {
            ("GET", RESOURCE_METADATA) => Some(Reply::json(
                "200 OK",
                json!({
                    "resource": format!("{base}/mcp"),
                    "authorization_servers": [issuer],
                    "scopes_supported": [SCOPE],
                }),
            )),
            ("GET", ISSUER_METADATA) => Some(Reply::json(
                "200 OK",
                json!({
                    "issuer": issuer,
                    "authorization_endpoint": format!("{issuer}/authorize"),
                    "token_endpoint": format!("{issuer}/token"),
                    "registration_endpoint": format!("{issuer}/register"),
                    "scopes_supported": [SCOPE, "offline_access"],
                    "code_challenge_methods_supported": ["S256"],
                    "token_endpoint_auth_methods_supported": ["none"],
                    "authorization_response_iss_parameter_supported": true,
                }),
            )),
            ("POST", "/auth/register") => Some(Reply::json(
                "201 Created",
                json!({ "client_id": "registered" }),
            )),
            ("GET", "/auth/authorize") => {
                let redirect = asked.field("redirect_uri")?;
                let state = asked.field("state")?;
                Some(Reply::status("302 Found").with_header(
                    "Location",
                    format!("{redirect}?code={CODE}&state={state}&iss={issuer}"),
                ))
            }
            ("POST", "/auth/token") => Some(self.grant(asked)),
            _ => None,
        }
    }

    fn grant(&mut self, asked: &Asked) -> Reply {
        let lifetime = match asked.field("grant_type").as_deref() {
            Some("authorization_code") if asked.field("code").as_deref() == Some(CODE) => {
                SHORT_LIVED
            }
            Some("refresh_token") if asked.field("refresh_token").is_some() => LONG_LIVED,
            _ => return Reply::json("400 Bad Request", json!({ "error": "invalid_grant" })),
        };

        let turn = self.issued.len() + 1;
        self.issued.push(format!("access-{turn}"));
        Reply::json(
            "200 OK",
            json!({
                "access_token": format!("access-{turn}"),
                "refresh_token": format!("refresh-{turn}"),
                "token_type": "Bearer",
                "expires_in": lifetime,
            }),
        )
    }
}
