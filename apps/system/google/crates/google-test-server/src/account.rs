use std::sync::Arc;

use google_account::{Endpoint, Grants, Master};

use crate::exchange::{Request, Response};
use crate::server::Server;

const AUTH: &str = "/auth";
const TOKEN: &str = "Bearer ya29.test";

#[derive(Debug)]
pub struct Account {
    pub server: Server,
}

impl Account {
    pub fn start(api: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Account {
        let server = Server::start(move |request| match request.path() {
            AUTH => match request.form().get("service").map(String::as_str) {
                Some("ac2dm") => Response::ok("Token=aas_et/test\nEmail=rikka@example.com\n"),
                _ => Response::ok("Auth=ya29.test\nExpiry=99999999999\n"),
            },
            _ if request.header("authorization") != Some(TOKEN) => Response::status(401),
            _ => api(request),
        });
        Account { server }
    }

    pub async fn grants(&self) -> Arc<Grants> {
        let endpoint = Endpoint::new(&self.server.url(AUTH)).expect("a local endpoint");
        let master = Master::sign_in(&endpoint, "oauth2_4/code")
            .await
            .expect("a master token");
        Arc::new(Grants::new(endpoint, master))
    }

    #[must_use]
    pub fn asked(&self, path: &str) -> Vec<Request> {
        self.server
            .requests()
            .into_iter()
            .filter(|request| request.path() == path)
            .collect()
    }

    #[must_use]
    pub fn calls(&self) -> Vec<Request> {
        self.server
            .requests()
            .into_iter()
            .filter(|request| request.path() != AUTH)
            .collect()
    }
}
