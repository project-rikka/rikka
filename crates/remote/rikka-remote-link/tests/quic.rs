use std::net::{Ipv6Addr, SocketAddr};

use rikka_remote_link::{
    LinkError, SERVER_NAME, ServerIdentity, Trust, client_config, client_endpoint, read_message,
    server_endpoint,
};
use rikka_remote_wire::{ClientMessage, ServerMessage};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn identities_persist_with_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote/identity.pem");
    let made = ServerIdentity::load_or_create(&path, "host").unwrap();
    let loaded = ServerIdentity::load_or_create(&path, "host").unwrap();
    assert_eq!(made.pin(), loaded.pin());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn clients_trust_on_first_use_and_refuse_a_changed_server() {
    let folder = tempfile::tempdir().unwrap();
    let identity = ServerIdentity::load_or_create(&folder.path().join("a.pem"), "host").unwrap();
    let impostor = ServerIdentity::load_or_create(&folder.path().join("b.pem"), "host").unwrap();
    runtime().block_on(async {
        let server = server_endpoint(&identity, 0).unwrap();
        let port = server.local_addr().unwrap().port();
        let address = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), port);
        let serving = tokio::spawn(async move {
            while let Some(incoming) = server.accept().await {
                let Ok(connection) = incoming.await else {
                    continue;
                };
                let (mut send, mut recv) = connection.accept_bi().await.unwrap();
                let (kind, body) = read_message(&mut recv).await.unwrap().unwrap();
                let hello = ClientMessage::decode(kind, &body).unwrap();
                assert!(matches!(hello, ClientMessage::Hello { .. }));
                let welcome = ServerMessage::Welcome {
                    version: 1,
                    host: "host".into(),
                    user: "user".into(),
                };
                send.write_all(&welcome.encode()).await.unwrap();
                send.finish().unwrap();
                connection.closed().await;
            }
        });

        let client = client_endpoint().unwrap();
        let first = Trust::first_use();
        let connection = client
            .connect_with(client_config(&first).unwrap(), address, SERVER_NAME)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(first.seen(), Some(identity.pin()));
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let hello = ClientMessage::Hello {
            version: 1,
            device: "test".into(),
            platform: "linux".into(),
        };
        send.write_all(&hello.encode()).await.unwrap();
        let (kind, body) = read_message(&mut recv).await.unwrap().unwrap();
        assert!(matches!(
            ServerMessage::decode(kind, &body).unwrap(),
            ServerMessage::Welcome { .. }
        ));
        connection.close(0_u32.into(), b"");

        let wrong = Trust::pinned(impostor.pin());
        let refused = client
            .connect_with(client_config(&wrong).unwrap(), address, SERVER_NAME)
            .unwrap()
            .await;
        assert!(matches!(
            refused.map_err(LinkError::from),
            Err(LinkError::Connection(_))
        ));
        assert!(wrong.mismatched());
        serving.abort();
    });
}
