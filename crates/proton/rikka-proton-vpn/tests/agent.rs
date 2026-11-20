use std::time::Duration;

use rikka_proton_vpn::{
    AgentCode, AgentConnection, AgentEvent, AgentFeatures, AgentState, AgentStatus, JailState,
    KeyPair, LocalAgent, NetShieldLevel,
};
use serde_json::json;

use crate::support::agent::{Pki, SERVER_NAME};

const TIMEOUT: Duration = Duration::from_secs(10);

async fn next_event(agent: &mut AgentConnection) -> AgentEvent {
    tokio::time::timeout(TIMEOUT, agent.next_event())
        .await
        .unwrap()
        .unwrap()
}

async fn wait_for_state(agent: &mut AgentConnection, expected: AgentState) {
    loop {
        if let AgentEvent::State(state) = next_event(agent).await
            && state == expected
        {
            return;
        }
    }
}

async fn next_status(agent: &mut AgentConnection) -> AgentStatus {
    loop {
        if let AgentEvent::Status(status) = next_event(agent).await {
            return *status;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_syncs_features_and_reports_status() {
    let pki = Pki::new();
    let keys = KeyPair::generate();
    let mut server = pki.agent().await;
    let mut agent = LocalAgent::new(&pki.certificate_for(&keys), &keys, SERVER_NAME)
        .address(server.address)
        .roots(pki.roots.clone())
        .features(
            AgentFeatures::new()
                .netshield(NetShieldLevel::Malware)
                .vpn_accelerator(true)
                .moderate_nat(true),
        )
        .connect()
        .unwrap();

    let mut peer = server.peer().await;
    peer.send(json!({"status": {
        "state": "connected",
        "features": {"netshield-level": 0, "split-tcp": true, "randomized-nat": true},
        "connection-details": {"device-ip": "1.2.3.4", "device-country": "CH", "server-ipv4": "5.6.7.8"}
    }}))
    .await;

    assert_eq!(
        peer.receive().await,
        json!({"features-set": {"netshield-level": 1, "randomized-nat": false}})
    );
    wait_for_state(&mut agent, AgentState::Connected).await;
    let reported = next_status(&mut agent).await;
    assert_eq!(reported.state, JailState::Connected);
    let details = reported.connection.unwrap();
    assert_eq!(details.device_ip.as_deref(), Some("1.2.3.4"));
    assert_eq!(details.server_ipv4.as_deref(), Some("5.6.7.8"));

    agent.request_status(true);
    assert_eq!(
        peer.receive().await,
        json!({"status-get": {"features-statistics": true}})
    );
    peer.send(json!({"status": {
        "state": "connected",
        "features": {"netshield-level": 1, "split-tcp": true, "randomized-nat": false},
        "features-statistics": {"netshield-level": {"DNSBL/1b": 1, "DNSBL/2a": 2, "DNSBL/2b": 3}}
    }}))
    .await;
    let stats = next_status(&mut agent).await.netshield.unwrap();
    assert_eq!((stats.malware, stats.ads, stats.trackers), (1, 2, 3));
    assert_eq!(stats.saved_bytes(), (750 + 2 * 200 + 3 * 50) * 1024);

    peer.send(json!({"error": {"code": 86111, "description": "too many sessions"}}))
        .await;
    loop {
        if let AgentEvent::Error(error) = next_event(&mut agent).await {
            assert_eq!(error.code, AgentCode::MaxSessionsFree);
            assert_eq!(error.description, "too many sessions");
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn jailed_partner_server_counts_as_connected() {
    let pki = Pki::new();
    let keys = KeyPair::generate();
    let mut server = pki.agent().await;
    let mut agent = LocalAgent::new(&pki.certificate_for(&keys), &keys, SERVER_NAME)
        .address(server.address)
        .roots(pki.roots.clone())
        .features(AgentFeatures::new().bouncing("partner"))
        .connect()
        .unwrap();

    let mut peer = server.peer().await;
    peer.send(json!({"status": {
        "state": "hard-jailed",
        "features": {"bouncing": "partner"},
        "reason": {"code": 86151, "final": true, "description": "low plan"}
    }}))
    .await;

    wait_for_state(&mut agent, AgentState::Connected).await;
    let reported = next_status(&mut agent).await;
    assert_eq!(reported.state, JailState::Connected);
    assert!(reported.reason.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_reconnects_after_drop() {
    let pki = Pki::new();
    let keys = KeyPair::generate();
    let mut server = pki.agent().await;
    let mut agent = LocalAgent::new(&pki.certificate_for(&keys), &keys, SERVER_NAME)
        .address(server.address)
        .roots(pki.roots.clone())
        .connect()
        .unwrap();

    let mut peer = server.peer().await;
    peer.send(json!({"status": {"state": "jailed"}})).await;
    wait_for_state(&mut agent, AgentState::SoftJailed).await;
    peer.hang_up().await;
    wait_for_state(&mut agent, AgentState::ConnectionError).await;

    let mut again = server.peer().await;
    again.send(json!({"status": {"state": "connected"}})).await;
    wait_for_state(&mut agent, AgentState::Connected).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_certificate_for_other_host() {
    let pki = Pki::new();
    let keys = KeyPair::generate();
    let server = pki.agent().await;
    let mut agent = LocalAgent::new(&pki.certificate_for(&keys), &keys, "other.protonvpn.net")
        .address(server.address)
        .roots(pki.roots.clone())
        .connect()
        .unwrap();

    wait_for_state(&mut agent, AgentState::ServerCertificateError).await;
    assert_eq!(agent.state(), AgentState::ServerCertificateError);

    let stranger = Pki::new();
    let mut strange = LocalAgent::new(&pki.certificate_for(&keys), &keys, SERVER_NAME)
        .address(server.address)
        .roots(stranger.roots.clone())
        .connect()
        .unwrap();
    wait_for_state(&mut strange, AgentState::ServerCertificateError).await;
}
