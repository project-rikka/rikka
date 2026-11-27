use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use google_account::{Endpoint, Master};
use google_test_server::{Request, Response, Server};

pub(crate) const MASTER_TOKEN: &str = "aas_et/master";

pub(crate) struct Google {
    pub(crate) server: Server,
    pub(crate) minted: Arc<AtomicUsize>,
}

impl Google {
    pub(crate) fn start() -> Google {
        let minted = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&minted);
        let server = Server::start(move |request| answer(request, &counter));
        Google { server, minted }
    }

    pub(crate) fn endpoint(&self) -> Endpoint {
        Endpoint::new(&self.server.url("/auth")).expect("a local endpoint")
    }

    pub(crate) async fn master(&self) -> Master {
        Master::sign_in(&self.endpoint(), "oauth2_4/code")
            .await
            .expect("a master token")
    }
}

fn answer(request: &Request, minted: &AtomicUsize) -> Response {
    match request.path() {
        "/auth" => auth(request, minted),
        "/userinfo" => match request.header("authorization") {
            Some("Bearer ya29.spent") => Response::status(401),
            Some(_) => Response::json(
                r#"{"sub":"1234","email":"rikka@example.com","name":"Rikka","picture":"https://example.com/a.png"}"#,
            ),
            None => Response::status(403),
        },
        _ => Response::status(404),
    }
}

fn auth(request: &Request, minted: &AtomicUsize) -> Response {
    let form = request.form();
    let field = |name: &str| form.get(name).map(String::as_str);
    if field("service") == Some("ac2dm") {
        return match field("Token") {
            Some("oauth2_4/code") => Response::ok(format!(
                "Token={MASTER_TOKEN}\nEmail=rikka@example.com\nfirstName=Rikka\n"
            )),
            _ => Response::status(403).body("Error=BadAuthentication\n"),
        };
    }
    if field("EncryptedPasswd") != Some(MASTER_TOKEN) {
        return Response::status(403).body("Error=BadAuthentication\n");
    }
    let count = minted.fetch_add(1, Ordering::AcqRel);
    let token = match count {
        0 => "ya29.spent".to_owned(),
        count => format!("ya29.fresh{count}"),
    };
    Response::ok(format!("Auth={token}\nExpiry=99999999999\n"))
}
