use std::path::Path;

use crate::error::{Error, Result};
use crate::tunnel::wireguard::{WireGuardPeer, WireGuardTunnel, interface_for};

const SUBJECT: &str = "a WireGuard configuration";

impl WireGuardTunnel {
    pub fn from_config(name: impl Into<String>, text: &str) -> Result<WireGuardTunnel> {
        let file = read(text);
        let interface = file
            .sections
            .iter()
            .find(|(section, _)| section == "interface")
            .map(|(_, values)| values)
            .ok_or_else(|| Error::invalid(SUBJECT, "it has no [Interface] section"))?;
        let private_key = value(interface, "privatekey")
            .ok_or_else(|| Error::invalid(SUBJECT, "the interface has no private key"))?;

        let mut tunnel = WireGuardTunnel::new(name, private_key)
            .addresses(list(value(interface, "address").unwrap_or_default()))
            .dns(list(value(interface, "dns").unwrap_or_default()));
        if let Some(port) = value(interface, "listenport").and_then(|port| port.parse().ok()) {
            tunnel = tunnel.listen_port(port);
        }
        if let Some(mtu) = value(interface, "mtu").and_then(|mtu| mtu.parse().ok()) {
            tunnel = tunnel.mtu(mtu);
        }

        let peers: Vec<WireGuardPeer> = file
            .sections
            .iter()
            .filter(|(section, _)| section == "peer")
            .filter_map(|(_, values)| peer(values))
            .collect();
        if peers.is_empty() {
            return Err(Error::invalid(SUBJECT, "it has no peer to connect to"));
        }
        Ok(peers.into_iter().fold(tunnel, WireGuardTunnel::peer))
    }

    pub(crate) fn from_file(path: &Path) -> Result<WireGuardTunnel> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| Error::invalid(SUBJECT, error.to_string()))?;
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let interface = interface_for(&stem);
        Ok(WireGuardTunnel::from_config(interface.clone(), &text)?.interface(interface))
    }
}

fn peer(values: &[(String, String)]) -> Option<WireGuardPeer> {
    let mut peer = WireGuardPeer::new(value(values, "publickey")?)
        .allowed_ips(list(value(values, "allowedips").unwrap_or_default()));
    if let Some(endpoint) = value(values, "endpoint") {
        peer = peer.endpoint(endpoint);
    }
    if let Some(keepalive) =
        value(values, "persistentkeepalive").and_then(|seconds| seconds.parse().ok())
    {
        peer = peer.keepalive(keepalive);
    }
    if let Some(preshared) = value(values, "presharedkey") {
        peer = peer.preshared_key(preshared);
    }
    Some(peer)
}

fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn value<'a>(values: &'a [(String, String)], key: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

struct File {
    sections: Vec<(String, Vec<(String, String)>)>,
}

fn read(text: &str) -> File {
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            sections.push((name.trim().to_lowercase(), Vec::new()));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some((_, values)) = sections.last_mut() else {
            continue;
        };
        values.push((key.trim().to_lowercase(), value.trim().to_owned()));
    }
    File { sections }
}
