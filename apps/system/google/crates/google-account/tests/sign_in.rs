use google_account::{Endpoint, Error, Master};

use crate::support::Google;

#[tokio::test]
async fn sign_in_cookie_becomes_master_token() {
    let google = Google::start();

    let master = google.master().await;

    assert_eq!(master.email(), "rikka@example.com");
    let asked = google.server.requests();
    let form = asked[0].form();
    assert_eq!(form["service"], "ac2dm");
    assert_eq!(form["Token"], "oauth2_4/code");
    assert_eq!(form["androidId"].len(), 16);
    assert!(!format!("{master:?}").contains("aas_et"));
}

#[tokio::test]
async fn refused_sign_in_reports_signed_out() {
    let google = Google::start();

    let refused = Master::sign_in(&google.endpoint(), "stale").await;

    assert!(matches!(refused, Err(Error::SignedOut)));
}

#[test]
fn endpoint_refuses_foreign_schemes() {
    assert!(matches!(
        Endpoint::new("ftp://example.com/auth"),
        Err(Error::Endpoint(_))
    ));
}
