use std::net::SocketAddr;
use std::sync::Arc;

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer,
    KeyPair as SigningPair,
};
use rikka_proton_vpn::{Certificate, KeyPair};
use rustls::RootCertStore;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ServerConfig, WebPkiClientVerifier};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

pub(crate) const SERVER_NAME: &str = "node-test.protonvpn.net";
const PENDING_SESSIONS: usize = 4;

pub(crate) struct Pki {
    pub(crate) roots: String,
    authority: Issuer<'static, SigningPair>,
    server: ServerConfig,
}

pub(crate) struct Agent {
    pub(crate) address: SocketAddr,
    sessions: mpsc::Receiver<TlsStream<tokio::net::TcpStream>>,
}

pub(crate) struct Peer(TlsStream<tokio::net::TcpStream>);

impl Pki {
    pub(crate) fn new() -> Pki {
        let authority_key = SigningPair::generate().unwrap();
        let mut authority = CertificateParams::new(Vec::<String>::new()).unwrap();
        authority
            .distinguished_name
            .push(DnType::CommonName, "Test VPN Root CA");
        authority.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let signed = authority.self_signed(&authority_key).unwrap();
        let (roots, authority_der) = (signed.pem(), signed.der().clone());
        let authority = Issuer::new(authority, authority_key);

        let server_key = SigningPair::generate().unwrap();
        let mut server = CertificateParams::new(vec![SERVER_NAME.to_owned()]).unwrap();
        server.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let server_der = server
            .signed_by(&server_key, &authority)
            .unwrap()
            .der()
            .clone();

        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut trusted = RootCertStore::empty();
        trusted.add(authority_der.clone()).unwrap();
        let verifier =
            WebPkiClientVerifier::builder_with_provider(Arc::new(trusted), provider.clone())
                .build()
                .unwrap();
        let server = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                vec![server_der],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(server_key.serialize_der())),
            )
            .unwrap();

        Pki {
            roots,
            authority,
            server,
        }
    }

    pub(crate) fn certificate_for(&self, keys: &KeyPair) -> Certificate {
        serde_json::from_value(self.certificate_json(keys)).unwrap()
    }

    pub(crate) fn certificate_json(&self, keys: &KeyPair) -> Value {
        let key = SigningPair::try_from(keys.private_key_der().as_slice()).unwrap();
        let mut client = CertificateParams::new(Vec::<String>::new()).unwrap();
        client.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let issued = client.signed_by(&key, &self.authority).unwrap();

        json!({
            "Code": 1000,
            "SerialNumber": "0x1",
            "ClientKeyFingerprint": keys.fingerprint(),
            "ClientKey": keys.public_key_pem(),
            "Certificate": issued.pem(),
            "ExpirationTime": 4_000_000_000_i64,
            "RefreshTime": 3_900_000_000_i64,
            "Mode": "session"
        })
    }

    pub(crate) async fn agent(&self) -> Agent {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(self.server.clone()));
        let (accepted, sessions) = mpsc::channel(PENDING_SESSIONS);

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let acceptor = acceptor.clone();
                let accepted = accepted.clone();
                tokio::spawn(async move {
                    if let Ok(session) = acceptor.accept(stream).await {
                        accepted.send(session).await.ok();
                    }
                });
            }
        });

        Agent { address, sessions }
    }
}

impl Agent {
    pub(crate) async fn peer(&mut self) -> Peer {
        Peer(self.sessions.recv().await.unwrap())
    }
}

impl Peer {
    pub(crate) async fn send(&mut self, message: Value) {
        let body = serde_json::to_vec(&message).unwrap();
        let length = u32::try_from(body.len()).unwrap();
        self.0.write_all(&length.to_be_bytes()).await.unwrap();
        self.0.write_all(&body).await.unwrap();
        self.0.flush().await.unwrap();
    }

    pub(crate) async fn receive(&mut self) -> Value {
        let length = self.0.read_u32().await.unwrap();
        let mut body = vec![0_u8; usize::try_from(length).unwrap()];
        self.0.read_exact(&mut body).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    pub(crate) async fn hang_up(mut self) {
        self.0.shutdown().await.ok();
    }
}
