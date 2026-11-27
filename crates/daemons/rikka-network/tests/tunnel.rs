use rikka_network::{Error, WireGuardTunnel};

const CONFIG: &str = "[Interface]
PrivateKey = MHyDhk8oM8tCei7xwAoBPP3/J2jZgMCjpSDwBpBN6U8=
Address = 10.2.0.2/32, 2a07:b944::2:2/128
DNS = 10.2.0.1 # the tunnel's resolver

[Peer]
PublicKey = hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 185.159.157.1:51820
PersistentKeepalive = 60
";

#[test]
fn imports_wg_quick_and_rejects_invalid() {
    let tunnel = WireGuardTunnel::from_config("Office VPN #1", CONFIG).unwrap();
    assert_eq!(tunnel.name(), "Office VPN #1");
    assert_eq!(tunnel.interface_name(), "OfficeVPN1");
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
