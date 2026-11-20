use rikka_oauth::{Challenge, Error, Metadata, ProtectedResource};
use serde_json::json;

use crate::support::Site;

#[test]
fn challenge_parameters_are_read() {
    let challenge = Challenge::parse(
        r#"Basic realm="files", Bearer error="insufficient_scope", scope="read \"all\" write", resource_metadata=https://mcp.example.com/.well-known/oauth-protected-resource, max_age=5"#,
    );

    assert_eq!(challenge.get("realm"), Some("files"));
    assert_eq!(challenge.scope(), Some("read \"all\" write"));
    assert_eq!(
        challenge.resource_metadata(),
        Some("https://mcp.example.com/.well-known/oauth-protected-resource")
    );
    assert_eq!(challenge.get("max_age"), Some("5"));
    assert!(challenge.is_insufficient_scope());

    let bare = Challenge::parse("Bearer");
    assert_eq!(bare.scope(), None);
    assert!(!bare.is_insufficient_scope());
}

#[tokio::test]
async fn protected_resource_falls_back_to_root_document() {
    let site = Site::start(|base, target| match target {
        "/.well-known/oauth-protected-resource" => Some(json!({
            "resource": base,
            "authorization_servers": [format!("{base}/tenant"), "not a url"],
            "scopes_supported": ["read", "write"],
        })),
        _ => None,
    })
    .await;
    let resource = format!("{}/mcp/v1?mode=fast", site.base);

    let described = ProtectedResource::discover(&resource)
        .await
        .expect("the site answered")
        .expect("the resource is described");

    assert_eq!(
        site.targets(),
        [
            "/.well-known/oauth-protected-resource/mcp/v1",
            "/.well-known/oauth-protected-resource",
        ]
    );
    let servers: Vec<&str> = described
        .authorization_servers()
        .iter()
        .map(|server| server.as_str())
        .collect();
    assert_eq!(servers, [format!("{}/tenant", site.base)]);
    assert_eq!(described.scopes_supported(), ["read", "write"]);
}

#[tokio::test]
async fn protected_resource_must_describe_requested_resource() {
    let site = Site::start(|base, target| match target {
        "/described" => Some(json!({ "resource": format!("{base}/other") })),
        "/elsewhere" => Some(json!({ "resource": "https://elsewhere.example.com/mcp" })),
        "/prefix" => Some(json!({ "resource": format!("{base}/mc") })),
        _ => None,
    })
    .await;
    let resource = format!("{}/mcp", site.base);

    for document in ["described", "elsewhere", "prefix"] {
        let mismatched =
            ProtectedResource::fetch(&format!("{}/{document}", site.base), &resource).await;
        assert!(
            matches!(mismatched, Err(Error::ResourceMismatch { .. })),
            "{document}: {mismatched:?}"
        );
    }
    let absent = ProtectedResource::fetch(&format!("{}/missing", site.base), &resource).await;
    assert!(matches!(absent, Ok(None)), "{absent:?}");
}

#[tokio::test]
async fn issuer_metadata_tries_rfc8414_then_openid_locations() {
    let site = Site::start(|base, target| match target {
        "/tenant/.well-known/openid-configuration" => Some(json!({
            "issuer": format!("{base}/tenant"),
            "authorization_endpoint": format!("{base}/tenant/authorize"),
            "token_endpoint": format!("{base}/tenant/token"),
            "registration_endpoint": format!("{base}/tenant/register"),
            "scopes_supported": ["read", "offline_access"],
            "code_challenge_methods_supported": ["plain"],
            "token_endpoint_auth_methods_supported": ["client_secret_post"],
            "authorization_response_iss_parameter_supported": true,
        })),
        "/.well-known/oauth-authorization-server/impostor" => Some(json!({
            "issuer": format!("{base}/somebody-else"),
            "token_endpoint": format!("{base}/token"),
        })),
        _ => None,
    })
    .await;
    let issuer = format!("{}/tenant", site.base);

    let metadata = Metadata::discover_issuer(&format!("{issuer}/"))
        .await
        .expect("the site answered")
        .expect("the issuer publishes metadata");

    assert_eq!(
        site.targets(),
        [
            "/.well-known/oauth-authorization-server/tenant",
            "/.well-known/openid-configuration/tenant",
            "/tenant/.well-known/openid-configuration",
        ]
    );
    assert_eq!(metadata.issuer(), Some(issuer.as_str()));
    assert_eq!(
        metadata.authorization_endpoint().map(|url| url.as_str()),
        Some(format!("{issuer}/authorize").as_str())
    );
    assert_eq!(
        metadata.registration_endpoint().map(|url| url.as_str()),
        Some(format!("{issuer}/register").as_str())
    );
    assert_eq!(metadata.scopes_supported(), ["read", "offline_access"]);
    assert!(!metadata.supports_s256());
    assert!(!metadata.supports_secret_basic());

    assert!(metadata.check_response_issuer(Some(&issuer)).is_ok());
    assert!(matches!(
        metadata.check_response_issuer(Some("https://attacker.example.com")),
        Err(Error::IssuerMismatch { .. })
    ));
    assert!(matches!(
        metadata.check_response_issuer(None),
        Err(Error::MissingIssuer)
    ));

    let impostor = Metadata::discover_issuer(&format!("{}/impostor", site.base)).await;
    assert!(
        matches!(impostor, Err(Error::IssuerMismatch { .. })),
        "{impostor:?}"
    );
    let unpublished = Metadata::discover_issuer(&format!("{}/nobody", site.base)).await;
    assert!(matches!(unpublished, Ok(None)), "{unpublished:?}");
}
