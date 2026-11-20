use std::collections::HashMap;
use std::fmt;
use std::fs::{DirBuilder, OpenOptions, Permissions};
use std::io::Write as _;
use std::net::Ipv6Addr;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::profile::dbus;
use crate::tunnel::addressing::EXCLUSIVE_DNS_PRIORITY;
use crate::tunnel::connection;
use crate::values::{self, Draft};

const SERVICE: &str = "org.freedesktop.NetworkManager.openvpn";
const VPN: &str = "vpn";
const SUBJECT: &str = "an OpenVPN tunnel";
const CERTIFICATES: [&str; 2] = [".cert", "nm-openvpn"];
const FALLBACK_BASENAME: &str = "openvpn";
const STORED_BY_NETWORK_MANAGER: &str = "0";
const NEVER_STORED: &str = "2";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OpenVpnTransport {
    #[default]
    Udp,
    Tcp,
}

#[must_use]
#[derive(Clone, PartialEq, Eq)]
pub struct OpenVpnTunnel {
    name: String,
    remotes: Vec<String>,
    transport: OpenVpnTransport,
    ca: String,
    tls_crypt: String,
    certificate: String,
    key: String,
    key_passphrase: String,
    verify_name: Option<String>,
    ipv6: bool,
}

pub(crate) struct Blobs {
    ca: PathBuf,
    tls_crypt: PathBuf,
    certificate: PathBuf,
    key: PathBuf,
}

impl Blobs {
    pub(crate) fn into_files(self) -> Vec<PathBuf> {
        vec![self.ca, self.tls_crypt, self.certificate, self.key]
    }
}
impl OpenVpnTunnel {
    pub fn new(name: impl Into<String>) -> OpenVpnTunnel {
        OpenVpnTunnel {
            name: name.into(),
            remotes: Vec::new(),
            transport: OpenVpnTransport::Udp,
            ca: String::new(),
            tls_crypt: String::new(),
            certificate: String::new(),
            key: String::new(),
            key_passphrase: String::new(),
            verify_name: None,
            ipv6: false,
        }
    }

    pub fn remote(mut self, host: impl AsRef<str>, port: u16) -> OpenVpnTunnel {
        let host = host.as_ref();
        let remote = match host.parse::<Ipv6Addr>() {
            Ok(_) => format!("[{host}]:{port}"),
            Err(_) => format!("{host}:{port}"),
        };
        self.remotes.push(remote);
        self
    }

    pub fn transport(mut self, transport: OpenVpnTransport) -> OpenVpnTunnel {
        self.transport = transport;
        self
    }

    pub fn ca(mut self, pem: impl Into<String>) -> OpenVpnTunnel {
        self.ca = pem.into();
        self
    }

    pub fn tls_crypt(mut self, key: impl Into<String>) -> OpenVpnTunnel {
        self.tls_crypt = key.into();
        self
    }

    pub fn certificate(mut self, pem: impl Into<String>) -> OpenVpnTunnel {
        self.certificate = pem.into();
        self
    }

    pub fn encrypted_key(
        mut self,
        pem: impl Into<String>,
        passphrase: impl Into<String>,
    ) -> OpenVpnTunnel {
        self.key = pem.into();
        self.key_passphrase = passphrase.into();
        self
    }

    pub fn verify_name(mut self, name: impl Into<String>) -> OpenVpnTunnel {
        self.verify_name = Some(name.into());
        self
    }

    pub fn ipv6(mut self, enabled: bool) -> OpenVpnTunnel {
        self.ipv6 = enabled;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn remotes(&self) -> &[String] {
        &self.remotes
    }

    pub(crate) fn blobs(&self) -> Result<Blobs> {
        self.check()?;
        let home = rikka_dirs::home()
            .ok_or_else(|| Error::invalid(SUBJECT, "this user has no home folder"))?;
        let folder = CERTIFICATES
            .iter()
            .fold(home.to_path_buf(), |folder, part| folder.join(part));
        DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(&folder)
            .map_err(|error| Error::invalid(SUBJECT, error.to_string()))?;

        let basename = basename(&self.name);
        let blob = |token: &str, contents: &str| -> Result<PathBuf> {
            let path = folder.join(format!("{basename}-{token}.pem"));
            write_blob(&path, contents)?;
            Ok(path)
        };
        Ok(Blobs {
            ca: blob("ca", &self.ca)?,
            tls_crypt: blob("tls-crypt", &self.tls_crypt)?,
            certificate: blob("cert", &self.certificate)?,
            key: blob("key", &self.key)?,
        })
    }

    pub(crate) fn draft(&self, user: &str, blobs: &Blobs) -> Result<Draft> {
        self.check()?;
        let mut data: HashMap<String, String> = [
            ("connection-type", "tls".to_owned()),
            ("remote", self.remotes.join(", ")),
            ("remote-random", "yes".to_owned()),
            ("cipher", "AES-256-GCM".to_owned()),
            ("dev", "tun".to_owned()),
            ("tunnel-mtu", "1500".to_owned()),
            ("mssfix", "0".to_owned()),
            ("reneg-seconds", "0".to_owned()),
            ("remote-cert-tls", "server".to_owned()),
            ("ca", path_text(&blobs.ca)?),
            ("tls-crypt", path_text(&blobs.tls_crypt)?),
            ("cert", path_text(&blobs.certificate)?),
            ("key", path_text(&blobs.key)?),
            ("cert-pass-flags", STORED_BY_NETWORK_MANAGER.to_owned()),
            ("challenge-response-flags", NEVER_STORED.to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
        if self.transport == OpenVpnTransport::Tcp {
            data.insert("proto-tcp".to_owned(), "yes".to_owned());
        }
        if self.ipv6 {
            data.insert("push-peer-info".to_owned(), "yes".to_owned());
        }
        if let Some(name) = &self.verify_name {
            data.insert("verify-x509-name".to_owned(), format!("name:{name}"));
        }
        let secrets = HashMap::from([("cert-pass".to_owned(), self.key_passphrase.clone())]);

        let mut draft = connection::draft(&self.name, VPN, None, user);
        values::put(&mut draft, VPN, "service-type", SERVICE.to_owned());
        values::put(&mut draft, VPN, "data", data);
        values::put(&mut draft, VPN, "secrets", secrets);
        values::put(&mut draft, dbus::IPV4, "method", "auto".to_owned());
        values::put(
            &mut draft,
            dbus::IPV4,
            "dns-priority",
            EXCLUSIVE_DNS_PRIORITY,
        );
        if self.ipv6 {
            values::put(&mut draft, dbus::IPV6, "method", "auto".to_owned());
            values::put(
                &mut draft,
                dbus::IPV6,
                "dns-priority",
                EXCLUSIVE_DNS_PRIORITY,
            );
        }
        Ok(draft)
    }

    fn check(&self) -> Result<()> {
        let missing = [
            (self.remotes.is_empty(), "it has no server to reach"),
            (self.ca.trim().is_empty(), "it has no certificate authority"),
            (self.tls_crypt.trim().is_empty(), "it has no tls-crypt key"),
            (
                self.certificate.trim().is_empty(),
                "it has no client certificate",
            ),
            (self.key.trim().is_empty(), "it has no client key"),
            (
                self.key_passphrase.is_empty(),
                "its client key has no passphrase",
            ),
        ];
        match missing.into_iter().find(|(lacking, _)| *lacking) {
            Some((_, detail)) => Err(Error::invalid(SUBJECT, detail)),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for OpenVpnTunnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenVpnTunnel")
            .field("name", &self.name)
            .field("remotes", &self.remotes)
            .field("transport", &self.transport)
            .field("verify_name", &self.verify_name)
            .field("ipv6", &self.ipv6)
            .finish_non_exhaustive()
    }
}

fn basename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(
            |character| match character.is_ascii_alphanumeric() || "-_".contains(character) {
                true => character,
                false => '_',
            },
        )
        .collect();
    let trimmed = cleaned.trim_matches('_');
    match trimmed.is_empty() {
        true => FALLBACK_BASENAME.to_owned(),
        false => trimmed.to_owned(),
    }
}

fn write_blob(path: &Path, contents: &str) -> Result<()> {
    let mut text = contents.trim().to_owned();
    text.push('\n');
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| {
            file.set_permissions(Permissions::from_mode(0o600))?;
            file.write_all(text.as_bytes())
        })
        .map_err(|error| Error::invalid(SUBJECT, format!("{}: {error}", path.display())))
}

fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| Error::invalid(SUBJECT, "the certificate folder is not UTF-8"))
}
