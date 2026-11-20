use rikka_proton_vpn::{KeyPair, WireGuardConfig, X25519PublicKey};

const PEER: &str = "hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=";

fn config() -> WireGuardConfig {
    let keys = KeyPair::from_seed([7_u8; 32]);
    let peer = X25519PublicKey::from_base64(PEER).unwrap();
    WireGuardConfig::new(&keys.x25519_secret_key(), &peer, "185.159.157.1:51820")
}

#[test]
fn wireguard_config_sets_address_and_dns() {
    let secret = KeyPair::from_seed([7_u8; 32])
        .x25519_secret_key()
        .to_base64();

    assert_eq!(
        config().to_string(),
        format!(
            "[Interface]\n\
             PrivateKey = {secret}\n\
             Address = 10.2.0.2/32\n\
             DNS = 10.2.0.1\n\
             \n\
             [Peer]\n\
             PublicKey = {PEER}\n\
             AllowedIPs = 0.0.0.0/0, ::/0\n\
             Endpoint = 185.159.157.1:51820\n\
             PersistentKeepalive = 60\n"
        )
    );

    let ipv6 = config().ipv6(true).to_string();
    assert!(ipv6.contains("Address = 10.2.0.2/32, 2a07:b944::2:2/128\n"));
    assert!(ipv6.contains("DNS = 10.2.0.1, 2a07:b944::2:1\n"));

    assert!(!format!("{:?}", config()).contains(&secret));
}
