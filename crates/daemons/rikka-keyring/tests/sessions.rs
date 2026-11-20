use std::collections::HashMap;

use rikka_secrets_protocol::WireSecret;
use rikka_secrets_protocol::names::{
    BUS_NAME, COLLECTION_INTERFACE, SERVICE_INTERFACE, SERVICE_PATH,
};
use tempfile::TempDir;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::support::{FIRST, Running, TOKEN, token};

async fn plain_session(connection: &zbus::Connection) -> zbus::Result<OwnedObjectPath> {
    let service = zbus::Proxy::new(connection, BUS_NAME, SERVICE_PATH, SERVICE_INTERFACE).await?;
    let (_, session): (OwnedValue, OwnedObjectPath) = service
        .call("OpenSession", &("plain", Value::from("")))
        .await?;
    Ok(session)
}

#[test]
fn session_serves_only_its_client() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs");
    zbus::block_on(running.secrets.store(&token(), "Test sign-in", TOKEN)).expect("secret stores");

    zbus::block_on(async {
        let owner = zbus::connection::Builder::session()?.build().await?;
        let stranger = zbus::connection::Builder::session()?.build().await?;
        let session = plain_session(&owner).await?;
        let service = zbus::Proxy::new(&stranger, BUS_NAME, SERVICE_PATH, SERVICE_INTERFACE).await?;
        let collection: OwnedObjectPath = service.call("ReadAlias", &("default",)).await?;
        let collection =
            zbus::Proxy::new(&stranger, BUS_NAME, collection, COLLECTION_INTERFACE).await?;
        let items: Vec<OwnedObjectPath> = collection
            .call("SearchItems", &(token().attributes(),))
            .await?;
        assert_eq!(items.len(), 1);

        let borrowed = service
            .call::<_, _, HashMap<OwnedObjectPath, WireSecret>>("GetSecrets", &(&items, &session))
            .await;
        assert!(
            matches!(&borrowed, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied"),
            "another client's session was accepted: {borrowed:?}"
        );

        let owner_service = zbus::Proxy::new(&owner, BUS_NAME, SERVICE_PATH, SERVICE_INTERFACE).await?;
        let own: HashMap<OwnedObjectPath, WireSecret> =
            owner_service.call("GetSecrets", &(&items, &session)).await?;
        assert_eq!(own.values().next().map(|secret| secret.value.as_slice()), Some(TOKEN));
        zbus::Result::Ok(())
    })
    .expect("bus calls run");
}
