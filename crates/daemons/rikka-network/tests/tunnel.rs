use rikka_network::{Error, OpenVpnTunnel, WireGuardTunnel};

const CONFIG: &str = "[Interface]
PrivateKey = MHyDhk8oM8tCei7xwAoBPP3/J2jZgMCjpSDwBpBN6U8=
Address = 10.2.0.2/32, 2a07:b944::2:2/128
DNS = 10.2.0.1 # Proton's resolver

[Peer]
PublicKey = hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 185.159.157.1:51820
PersistentKeepalive = 60
";

#[test]
fn imports_wg_quick_and_rejects_invalid() {
    let tunnel = WireGuardTunnel::from_config("Proton VPN CH#1", CONFIG).unwrap();
    assert_eq!(tunnel.name(), "Proton VPN CH#1");
    assert_eq!(tunnel.interface_name(), "ProtonVPNCH1");
    let peer = &tunnel.peers()[0];
    assert_eq!(
        peer.public_key(),
        "hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo="
    );
    assert_eq!(peer.endpoint_address(), Some("185.159.157.1:51820"));
    assert!(!format!("{tunnel:?}").contains("MHyDhk8o"));

    for broken in [
        "[Peer]\nPublicKey = hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=\n",
        "[Interface]\nAddress = 10.2.0.2/32\n[Peer]\nPublicKey = a\n",
        "[Interface]\nPrivateKey = MHyDhk8oM8tCei7xwAoBPP3/J2jZgMCjpSDwBpBN6U8=\n",
    ] {
        assert!(matches!(
            WireGuardTunnel::from_config("wg0", broken),
            Err(Error::Invalid { .. })
        ));
    }
}

#[test]
fn openvpn_remotes_match_plugin_format() {
    let tunnel = OpenVpnTunnel::new("Proton VPN CH#1")
        .encrypted_key("-----BEGIN ENCRYPTED PRIVATE KEY-----", "secret")
        .remote("185.159.157.1", 80)
        .remote("2a07:b944::2:1", 443)
        .remote("node-ch-01.protonvpn.net", 1194);

    assert_eq!(
        tunnel.remotes(),
        [
            "185.159.157.1:80",
            "[2a07:b944::2:1]:443",
            "node-ch-01.protonvpn.net:1194"
        ]
    );
    assert!(!format!("{tunnel:?}").contains("secret"));
}
