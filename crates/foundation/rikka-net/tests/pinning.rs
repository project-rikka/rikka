use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, PublicKeyData,
};
use rikka_net::{Certificate, Client, Error, RetryPolicy, SpkiPin};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

const HOST: &str = "pinned.test";
const ANSWER: &[u8] = b"pinned";

struct Authority {
    root: Certificate,
    root_pin: SpkiPin,
    leaf_pin: SpkiPin,
    chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl Authority {
    fn issuing(names: &[&str]) -> Authority {
        let mut root_params = CertificateParams::new(Vec::<String>::new()).expect("no names");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        root_params
            .distinguished_name
            .push(DnType::CommonName, "Rikka pinning root");
        let root_key = KeyPair::generate().expect("a root key is made");
        let root_pin = SpkiPin::from_public_key_info(&root_key.subject_public_key_info());
        let root = CertifiedIssuer::self_signed(root_params, root_key).expect("the root signs");

        let leaf_key = KeyPair::generate().expect("a leaf key is made");
        let leaf_pin = SpkiPin::from_public_key_info(&leaf_key.subject_public_key_info());
        let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
        let leaf = CertificateParams::new(names)
            .expect("the names are valid")
            .signed_by(&leaf_key, &root)
            .expect("the root signs the leaf");

        Authority {
            root: Certificate::from_der(root.der().to_vec()).expect("the root is DER"),
            root_pin,
            leaf_pin,
            chain: vec![leaf.der().clone()],
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
        }
    }

    fn serve(self) -> SocketAddr {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("the versions are supported")
            .with_no_client_auth()
            .with_single_cert(self.chain, self.key)
            .expect("the leaf and its key match");
        let config = Arc::new(config);

        let listener = TcpListener::bind("127.0.0.1:0").expect("a port is free");
        let address = listener.local_addr().expect("the socket has an address");

        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let config = Arc::clone(&config);
                std::thread::spawn(move || answer(config, stream));
            }
        });

        address
    }
}

fn answer(config: Arc<rustls::ServerConfig>, stream: TcpStream) {
    let Ok(connection) = rustls::ServerConnection::new(config) else {
        return;
    };
    let mut tls = BufReader::new(rustls::StreamOwned::new(connection, stream));

    let mut line = String::new();
    while tls.read_line(&mut line).is_ok_and(|read| read > 0) {
        if line == "\r\n" {
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                ANSWER.len()
            );
            let stream = tls.get_mut();
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(ANSWER);
            let _ = stream.flush();
            stream.conn.send_close_notify();
            let _ = stream.flush();
            return;
        }
        line.clear();
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Chain,
    Leaf,
}

#[derive(Debug, Clone, Copy)]
enum Pinned {
    Leaf,
    Root,
    Stranger,
}

async fn reach(names: &[&str], mode: Mode, pinned: Pinned) -> Result<Vec<u8>, Error> {
    let authority = Authority::issuing(names);
    let root = authority.root.clone();
    let pin = match pinned {
        Pinned::Leaf => authority.leaf_pin,
        Pinned::Root => authority.root_pin,
        Pinned::Stranger => SpkiPin::from_sha256([7; 32]),
    };
    let address = authority.serve();

    let builder = Client::builder()
        .retry(RetryPolicy::never())
        .resolve(HOST, address);
    let client = match mode {
        Mode::Chain => builder
            .only_root_certificates([root])
            .pin_public_keys([pin]),
        Mode::Leaf => builder.trust_leaf_public_keys([pin]),
    }
    .build()
    .expect("the client builds");

    let response = client
        .get(format!("https://{HOST}:{}/", address.port()))
        .send()
        .await?;
    Ok(response.bytes().await?.to_vec())
}

#[tokio::test(flavor = "multi_thread")]
async fn connects_only_with_matching_pin() {
    let cases = [
        (&[HOST][..], Mode::Chain, Pinned::Leaf, true),
        (&[HOST][..], Mode::Chain, Pinned::Root, true),
        (&[HOST][..], Mode::Chain, Pinned::Stranger, false),
        (&["elsewhere.test"][..], Mode::Chain, Pinned::Leaf, false),
        (&["elsewhere.test"][..], Mode::Leaf, Pinned::Leaf, true),
        (&["elsewhere.test"][..], Mode::Leaf, Pinned::Root, false),
        (&[HOST][..], Mode::Leaf, Pinned::Stranger, false),
    ];

    for (names, mode, pinned, holds) in cases {
        let reached = reach(names, mode, pinned).await;
        match holds {
            true => assert_eq!(
                reached.expect("the pinned connection holds"),
                ANSWER,
                "{names:?} {mode:?} {pinned:?}"
            ),
            false => assert!(
                matches!(reached, Err(Error::Tls(_))),
                "{names:?} {mode:?} {pinned:?} gave {reached:?}"
            ),
        }
    }
}

#[test]
fn pin_is_spki_sha256() {
    let key = KeyPair::generate().expect("a key is made");
    let certificate = CertificateParams::new(vec![HOST.to_owned()])
        .expect("the name is valid")
        .self_signed(&key)
        .expect("the key signs");
    let certificate = Certificate::from_der(certificate.der().to_vec()).expect("it is DER");

    let pin = SpkiPin::from_certificate(&certificate).expect("the certificate is readable");

    assert_eq!(
        pin,
        SpkiPin::from_public_key_info(&key.subject_public_key_info())
    );
    assert_eq!(
        SpkiPin::from_base64(pin.to_string()).expect("the pin reads back"),
        pin
    );
    assert_eq!(
        SpkiPin::from_base64(pin.to_base64()).expect("the bare pin reads back"),
        pin
    );
    assert!(SpkiPin::from_base64("c2hvcnQ=").is_err());
}
