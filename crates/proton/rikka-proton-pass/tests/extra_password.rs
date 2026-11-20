use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rikka_proton_pass::Error;
use serde_json::{Value, json};

use crate::support::fixture;
use rikka_test_proton::{FakeSrp, Request, SIGNED_MODULUS, response, success};

#[tokio::test(flavor = "multi_thread")]
async fn extra_password_uses_srp() {
    let registered: Arc<Mutex<Option<Value>>> = Arc::default();
    let server: Arc<Mutex<Option<FakeSrp>>> = Arc::default();
    let lying = Arc::new(AtomicBool::new(false));
    let without_session = Arc::new(AtomicBool::new(false));
    let proofs_sent = Arc::new(AtomicUsize::new(0));

    let fixture = {
        let (registered, server, lying) = (registered.clone(), server.clone(), lying.clone());
        let (without_session, proofs_sent) = (without_session.clone(), proofs_sent.clone());
        fixture(move |request: &Request| match request.route() {
            ("POST", "/auth/v4/sessions") => Some(response(200,
                r#"{"Code":1000,"UID":"unauth-session","AccessToken":"unauth-access","RefreshToken":"unauth-refresh","TokenType":"Bearer","Scopes":[]}"#,
            )),
            ("GET", "/auth/v4/modulus") => Some(response(200,
                json!({"Code": 1000, "ModulusID": "modulus-value", "Modulus": SIGNED_MODULUS})
                    .to_string(),
            )),
            ("POST", "/pass/v1/user/srp") => {
                *registered.lock().unwrap() = Some(request.json());
                Some(success())
            }
            ("GET", "/pass/v1/user/srp") => {
                let stored = registered.lock().unwrap().is_some();
                Some(response(200, json!({"Code": 1000, "HasSRP": stored}).to_string()))
            }
            ("GET", "/pass/v1/user/srp/info") => {
                let stored = registered.lock().unwrap().clone()?;
                let fake = FakeSrp::registered(stored["SrpSalt"].as_str()?, stored["SrpVerifier"].as_str()?);
                let ephemeral = fake.challenge();
                *server.lock().unwrap() = Some(fake);
                let mut data = json!({
                    "Modulus": SIGNED_MODULUS,
                    "ServerEphemeral": ephemeral,
                    "SrpSessionID": "srp-session",
                    "SrpSalt": stored["SrpSalt"],
                    "Version": 4
                });
                if without_session.load(Ordering::SeqCst) {
                    data.as_object_mut()?.remove("SrpSessionID");
                }
                Some(response(200, json!({"Code": 1000, "SRPData": data}).to_string()))
            }
            ("POST", "/pass/v1/user/srp/auth") => {
                proofs_sent.fetch_add(1, Ordering::SeqCst);
                let sent = request.json();
                assert_eq!(sent["SrpSessionID"], "srp-session");
                let proven = server.lock().unwrap().as_ref()?.prove(
                    sent["ClientEphemeral"].as_str()?,
                    sent["ClientProof"].as_str()?,
                );
                Some(match (proven, lying.load(Ordering::SeqCst)) {
                    (Some(_), true) => {
                        response(200, json!({"Code": 1000, "ServerProof": "AAAA"}).to_string())
                    }
                    (Some(proof), false) => {
                        response(200, json!({"Code": 1000, "ServerProof": proof}).to_string())
                    }
                    (None, _) => response(
                        422,
                        r#"{"Code":2011,"Error":"Wrong password"}"#,
                    ),
                })
            }
            _ => None,
        })
        .await
    };

    assert!(!fixture.client.has_extra_password().await.unwrap());

    fixture
        .client
        .set_extra_password("an extra secret")
        .await
        .unwrap();
    let sent = registered.lock().unwrap().clone().unwrap();
    assert_eq!(sent["SrpModulusID"], "modulus-value");
    assert!(fixture.client.has_extra_password().await.unwrap());

    fixture
        .client
        .unlock_with_extra_password("an extra secret")
        .await
        .unwrap();

    let wrong = fixture
        .client
        .unlock_with_extra_password("a wrong secret")
        .await
        .unwrap_err();
    assert!(matches!(wrong, Error::Api(_)));

    lying.store(true, Ordering::SeqCst);
    let forged = fixture
        .client
        .unlock_with_extra_password("an extra secret")
        .await
        .unwrap_err();
    assert!(matches!(
        forged,
        Error::Pgp(rikka_proton_pgp::Error::ServerProof)
    ));

    lying.store(false, Ordering::SeqCst);
    without_session.store(true, Ordering::SeqCst);
    let proofs_before = proofs_sent.load(Ordering::SeqCst);
    let missing = fixture
        .client
        .unlock_with_extra_password("an extra secret")
        .await
        .unwrap_err();
    assert!(matches!(missing, Error::MissingSrpSession));
    assert_eq!(proofs_sent.load(Ordering::SeqCst), proofs_before);
}
