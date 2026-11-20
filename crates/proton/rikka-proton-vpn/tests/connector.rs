use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream;
use rikka_proton_vpn::{
    AgentCode, ConnectionState, Connector, Credentials, Error, KeyPair, Problem, Protocol, Result,
    ServerQuery, Transmission, TunnelControl, TunnelEvent, TunnelEvents, TunnelPlan, VpnClient,
    VpnConnection,
};
use serde_json::json;

use crate::support::agent::{Agent, Pki, SERVER_NAME};
use crate::support::{PEER_KEY, logical};
use rikka_test_proton::{Record, Request, api, not_found, response};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
struct Tunnels {
    opened_tunnels: Arc<Mutex<Vec<(String, TunnelPlan)>>>,
    renewed: Arc<Mutex<Vec<TunnelPlan>>>,
    closed: Arc<Mutex<Vec<String>>>,
}

impl TunnelControl for Tunnels {
    type Tunnel = String;

    fn supports(&self, protocol: Protocol) -> bool {
        protocol != Protocol::OpenVpnTcp
    }

    async fn open(&self, name: &str, plan: &TunnelPlan) -> Result<(String, TunnelEvents)> {
        self.opened_tunnels
            .lock()
            .unwrap()
            .push((name.to_owned(), plan.clone()));
        let events = stream::iter([TunnelEvent::Connecting, TunnelEvent::Up])
            .chain(stream::pending())
            .boxed();
        Ok((name.to_owned(), events))
    }

    async fn renew(&self, _tunnel: &String, _name: &str, plan: &TunnelPlan) -> Result<()> {
        self.renewed.lock().unwrap().push(plan.clone());
        Ok(())
    }

    async fn close(&self, tunnel: String) -> Result<()> {
        self.closed.lock().unwrap().push(tunnel);
        Ok(())
    }
}

struct Harness {
    connector: Connector<Tunnels>,
    tunnels: Tunnels,
    agent: Agent,
    record: Record,
    keys: KeyPair,
}

async fn start_harness(tier: u8) -> Harness {
    let pki = Arc::new(Pki::new());
    let keys = KeyPair::generate();
    let issuer = (pki.clone(), keys.clone());
    let record = Record::default();
    let noted = record.clone();
    let api = api(move |request: Request| {
        noted.push(&request);
        match request.route() {
            ("GET", "/vpn/v2") => response(
                200,
                json!({"Code": 1000, "VPN": {
                    "Name": "person",
                    "Password": "tunnel-password",
                    "MaxTier": tier,
                    "MaxConnect": 10
                }})
                .to_string(),
            ),
            ("GET", "/vpn/v2/clientconfig") => response(200, r#"{"Code":1000}"#),
            ("POST", "/vpn/v1/certificate") => {
                response(200, issuer.0.certificate_json(&issuer.1).to_string())
            }
            _ => not_found(),
        }
    })
    .await;

    let agent = pki.agent().await;
    let tunnels = Tunnels::default();
    let connector = Connector::new(VpnClient::new(api), tunnels.clone())
        .credentials(Credentials::new(keys.clone()))
        .agent_address(agent.address)
        .agent_roots(pki.roots.clone());

    Harness {
        connector,
        tunnels,
        agent,
        record,
        keys,
    }
}

fn decrypt_key(pem: &str, passphrase: &str) -> Vec<u8> {
    let (label, der) = pkcs8::der::pem::decode_vec(pem.as_bytes()).unwrap();
    assert_eq!(label, "ENCRYPTED PRIVATE KEY");
    pkcs8::EncryptedPrivateKeyInfoRef::try_from(der.as_slice())
        .unwrap()
        .decrypt(passphrase)
        .unwrap()
        .as_bytes()
        .to_vec()
}

async fn wait_for_state(
    connection: &mut VpnConnection,
    predicate: impl Fn(&ConnectionState) -> bool,
) {
    let mut watched = connection.watch();
    tokio::time::timeout(TIMEOUT, watched.wait_for(|state| predicate(state)))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn connection_lifecycle() {
    let mut stage = start_harness(2).await;
    let servers = [logical(json!({}))];

    let mut connection = stage
        .connector
        .connect(&servers, &ServerQuery::fastest())
        .await
        .unwrap();

    let mut peer = stage.agent.peer().await;
    peer.send(json!({"status": {
        "state": "connected",
        "connection-details": {"device-ip": "1.2.3.4", "server-ipv4": "5.6.7.8"}
    }}))
    .await;
    wait_for_state(&mut connection, |state| match state {
        ConnectionState::Connected {
            status: Some(status),
            ..
        } => status.connection.is_some(),
        _ => false,
    })
    .await;

    let ConnectionState::Connected { server, .. } = connection.state() else {
        panic!("expected connected state");
    };
    assert_eq!(server.name, "CH#1");
    assert_eq!(server.entry_ip, "185.159.157.1");
    assert_eq!(server.port, 443);
    let opened_tunnels = stage.tunnels.opened_tunnels.lock().unwrap().clone();
    assert_eq!(opened_tunnels[0].0, "Proton VPN CH#1");
    let TunnelPlan::WireGuard(config) = &opened_tunnels[0].1 else {
        panic!("expected WireGuard tunnel");
    };
    assert_eq!(config.endpoint(), "185.159.157.1:443");
    assert_eq!(
        stage.record.matching("POST", "/vpn/v1/certificate").len(),
        1
    );

    connection.disconnect().await;
    assert_eq!(
        stage.tunnels.closed.lock().unwrap().clone(),
        vec!["Proton VPN CH#1".to_owned()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fatal_agent_error_closes_tunnel() {
    let mut stage = start_harness(2).await;
    let servers = [logical(json!({}))];

    let mut connection = stage
        .connector
        .connect(&servers, &ServerQuery::fastest())
        .await
        .unwrap();
    let mut peer = stage.agent.peer().await;
    peer.send(json!({"status": {"state": "connected"}})).await;
    peer.send(json!({"error": {"code": 86112, "description": "too many sessions"}}))
        .await;

    wait_for_state(&mut connection, ConnectionState::is_finished).await;
    assert_eq!(
        connection.state(),
        ConnectionState::Failed {
            problem: Problem::MaxSessions(AgentCode::MaxSessionsBasic)
        }
    );
    assert_eq!(stage.tunnels.closed.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unmatched_server_or_tier() {
    let stage = start_harness(0).await;
    let servers = [logical(json!({}))];

    let refused = [
        stage
            .connector
            .connect(&servers, &ServerQuery::fastest().country("JP"))
            .await,
        stage
            .connector
            .connect(&servers, &ServerQuery::fastest())
            .await,
        stage
            .connector
            .connect(
                &servers,
                &ServerQuery::fastest().protocol(Protocol::OpenVpnTcp),
            )
            .await,
    ];

    assert!(matches!(refused[0], Err(Error::NoServer)));
    assert!(matches!(refused[1], Err(Error::InsufficientTier { .. })));
    assert!(matches!(refused[2], Err(Error::UnsupportedProtocol { .. })));
    assert!(stage.tunnels.opened_tunnels.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn openvpn_renews_certificate() {
    let mut stage = start_harness(2).await;
    let servers = [logical(json!({}))];

    let mut connection = stage
        .connector
        .connect(
            &servers,
            &ServerQuery::fastest().protocol(Protocol::OpenVpnUdp),
        )
        .await
        .unwrap();
    let mut peer = stage.agent.peer().await;
    peer.send(json!({"status": {"state": "connected"}})).await;
    wait_for_state(&mut connection, ConnectionState::is_connected).await;

    let opened_tunnels = stage.tunnels.opened_tunnels.lock().unwrap().clone();
    let TunnelPlan::OpenVpn(config) = &opened_tunnels[0].1 else {
        panic!("expected OpenVPN tunnel");
    };
    assert_eq!(config.server_ip(), "185.159.157.1");
    assert_eq!(config.remote_ports(), [80, 51820, 4569, 1194, 5060]);
    assert!(!config.uses_tcp());
    assert_eq!(config.domain(), SERVER_NAME);
    assert!(config.ca().contains("BEGIN CERTIFICATE"));
    assert!(config.tls_crypt().contains("BEGIN OpenVPN Static key V1"));
    let issued = stage.record.matching("POST", "/vpn/v1/certificate");
    assert_eq!(issued.len(), 1);
    assert!(config.certificate().contains("BEGIN CERTIFICATE"));
    let passphrase = config.key_passphrase().expose();
    assert_eq!(passphrase.len(), 32);
    assert!(
        passphrase
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert_eq!(
        decrypt_key(config.encrypted_key(), passphrase),
        stage.keys.private_key_der()
    );

    peer.send(json!({"error": {"code": 86101, "description": "certificate expired"}}))
        .await;
    let mut again = stage.agent.peer().await;
    again.send(json!({"status": {"state": "connected"}})).await;
    let renewed = stage.tunnels.renewed.lock().unwrap().clone();
    let TunnelPlan::OpenVpn(fresh) = &renewed[0] else {
        panic!("expected renewed OpenVPN files");
    };
    assert_ne!(fresh.certificate(), config.certificate());
    assert_eq!(fresh.key_passphrase().expose(), passphrase);
    assert_eq!(
        decrypt_key(fresh.encrypted_key(), passphrase),
        stage.keys.private_key_der()
    );
    assert_eq!(
        stage.record.matching("POST", "/vpn/v1/certificate").len(),
        2
    );

    connection.disconnect().await;
    assert_eq!(stage.tunnels.closed.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn stealth_uses_wireguard_over_tls() {
    let mut stage = start_harness(2).await;
    let servers = [logical(json!({}))];

    let mut connection = stage
        .connector
        .connect(
            &servers,
            &ServerQuery::fastest().protocol(Protocol::WireGuardTls),
        )
        .await
        .unwrap();
    let mut peer = stage.agent.peer().await;
    peer.send(json!({"status": {"state": "connected"}})).await;
    wait_for_state(&mut connection, ConnectionState::is_connected).await;

    let opened_tunnels = stage.tunnels.opened_tunnels.lock().unwrap().clone();
    let TunnelPlan::Protun(config) = &opened_tunnels[0].1 else {
        panic!("expected ProTun tunnel");
    };
    assert_eq!(config.peer(), "CH#1");
    assert_eq!(config.server_ip(), "185.159.157.1");
    assert_eq!(config.transmission(), Transmission::Tls);
    assert_eq!(config.transport_ports(), [443]);
    assert_eq!(config.wireguard().public_key().to_base64(), PEER_KEY);
    assert_eq!(
        stage.record.matching("POST", "/vpn/v1/certificate").len(),
        1
    );

    connection.disconnect().await;
    assert_eq!(stage.tunnels.closed.lock().unwrap().len(), 1);
}
