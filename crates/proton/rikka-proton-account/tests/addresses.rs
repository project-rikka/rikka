use crate::support::fixture::{EMAIL, Server, address, ok};
use rikka_proton_account::AddressId;
use rikka_test_proton::response;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn address_management_uses_address_routes() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|_, request| match request.route() {
            ("POST", "/core/v4/addresses") | ("POST", "/core/v4/addresses/setup") => {
                Some(response(
                    200,
                    json!({"Code": 1000, "Address": address("new-address", EMAIL, Vec::new())})
                        .to_string(),
                ))
            }
            ("PUT", _) => Some(ok()),
            _ => None,
        })
        .await;
    let id = AddressId::from("new-address");

    let created = account
        .create_address("proton.test")
        .local_part("tester")
        .display_name("Tester")
        .execute()
        .await
        .unwrap();
    assert_eq!(created.id(), &id);
    account
        .create_address("proton.test")
        .display_name("Tester")
        .execute()
        .await
        .unwrap();
    account
        .reorder_addresses(&[id.clone(), AddressId::from("old-address")])
        .await
        .unwrap();
    account
        .update_address(&id)
        .display_name("New name")
        .signature("<p>Bye</p>")
        .execute()
        .await
        .unwrap();
    account.disable_address(&id).await.unwrap();
    account.enable_address(&id).await.unwrap();
    account.delete_address(&id).await.unwrap();

    let expected = [
        (
            "POST",
            "/core/v4/addresses",
            json!({"Local": "tester", "Domain": "proton.test", "DisplayName": "Tester"}),
        ),
        (
            "POST",
            "/core/v4/addresses/setup",
            json!({"Domain": "proton.test", "DisplayName": "Tester"}),
        ),
        (
            "PUT",
            "/core/v4/addresses/order",
            json!({"AddressIDs": ["new-address", "old-address"]}),
        ),
        (
            "PUT",
            "/core/v4/addresses/new-address",
            json!({"DisplayName": "New name", "Signature": "<p>Bye</p>"}),
        ),
    ];
    for (method, path, body) in expected {
        assert_eq!(
            server.last_body(method, path),
            Some(body),
            "{method} {path}"
        );
    }

    let paths = server.request_paths();
    for action in ["disable", "enable", "delete"] {
        let path = format!("/core/v4/addresses/new-address/{action}");
        assert!(paths.contains(&("PUT".to_owned(), path.clone())), "{path}");
    }
}
