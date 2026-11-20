use rikka_proton_api::{Api, Session};

#[test]
fn debug_output_redacts_tokens() {
    let session = Session::new("uid-value", "access-value", "refresh-value");
    let debug = format!("{session:?}");
    for secret in ["uid-value", "access-value", "refresh-value"] {
        assert!(!debug.contains(secret), "{debug}");
    }

    let api = Api::builder()
        .app_version("linux-pass@1.0.0")
        .build()
        .unwrap();
    api.set_session(session).unwrap();
    let debug = format!("{api:?}");
    for secret in ["access-value", "refresh-value"] {
        assert!(!debug.contains(secret), "{debug}");
    }
}
