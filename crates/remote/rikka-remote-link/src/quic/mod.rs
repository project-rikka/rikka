mod controller;
mod identity;
mod verify;

use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6};
use std::sync::Arc;
use std::time::Duration;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Endpoint, EndpointConfig, IdleTimeout, TransportConfig, VarInt};
use rikka_remote_wire::{ALPN, HEADER_BYTES, Header};
use socket2::{Domain, Protocol, Socket, Type};

use crate::error::LinkError;

pub use identity::{Pin, ServerIdentity};
pub use verify::Trust;

use controller::ApplicationPaced;
use verify::PinVerifier;

pub const SERVER_NAME: &str = "rikka-remote";
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
pub const KEEP_ALIVE: Duration = Duration::from_secs(1);
const DATAGRAM_BUFFER: usize = 8 << 20;
const SOCKET_BUFFER: usize = 8 << 20;
#[cfg(unix)]
const MEDIA_CLASS: u32 = 0x88;
const INITIAL_RTT: Duration = Duration::from_millis(30);
const UPLOAD_STREAMS: u32 = 16;

pub fn server_endpoint(identity: &ServerIdentity, port: u16) -> Result<Endpoint, LinkError> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut tls = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![identity.certificate()], identity.key())?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto =
        QuicServerConfig::try_from(tls).map_err(|error| LinkError::Quic(error.to_string()))?;
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    let mut transport = transport()?;
    transport
        .congestion_controller_factory(Arc::new(ApplicationPaced))
        .max_concurrent_bidi_streams(VarInt::from_u32(1))
        .max_concurrent_uni_streams(VarInt::from_u32(UPLOAD_STREAMS));
    config.transport_config(Arc::new(transport));
    let socket = socket(port)?;
    Endpoint::new(
        EndpointConfig::default(),
        Some(config),
        socket,
        Arc::new(quinn::TokioRuntime),
    )
    .map_err(LinkError::Endpoint)
}

pub fn client_endpoint() -> Result<Endpoint, LinkError> {
    Endpoint::new(
        EndpointConfig::default(),
        None,
        socket(0)?,
        Arc::new(quinn::TokioRuntime),
    )
    .map_err(LinkError::Endpoint)
}

pub fn client_config(trust: &Trust) -> Result<quinn::ClientConfig, LinkError> {
    let provider = rustls::crypto::aws_lc_rs::default_provider();
    let algorithms = provider.signature_verification_algorithms;
    let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinVerifier::new(trust.clone(), algorithms)))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto =
        QuicClientConfig::try_from(tls).map_err(|error| LinkError::Quic(error.to_string()))?;
    let mut config = quinn::ClientConfig::new(Arc::new(crypto));
    let mut transport = transport()?;
    transport
        .keep_alive_interval(Some(KEEP_ALIVE))
        .max_concurrent_bidi_streams(VarInt::from_u32(0))
        .max_concurrent_uni_streams(VarInt::from_u32(0));
    config.transport_config(Arc::new(transport));
    Ok(config)
}

fn transport() -> Result<TransportConfig, LinkError> {
    let mut transport = TransportConfig::default();
    let idle =
        IdleTimeout::try_from(IDLE_TIMEOUT).map_err(|error| LinkError::Quic(error.to_string()))?;
    transport
        .max_idle_timeout(Some(idle))
        .datagram_receive_buffer_size(Some(DATAGRAM_BUFFER))
        .datagram_send_buffer_size(DATAGRAM_BUFFER)
        .initial_rtt(INITIAL_RTT);
    Ok(transport)
}

fn socket(port: u16) -> Result<std::net::UdpSocket, LinkError> {
    let socket =
        Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)).map_err(LinkError::Endpoint)?;
    socket.set_only_v6(false).map_err(LinkError::Endpoint)?;
    let tuned = [
        ("send buffer", socket.set_send_buffer_size(SOCKET_BUFFER)),
        ("receive buffer", socket.set_recv_buffer_size(SOCKET_BUFFER)),
        #[cfg(unix)]
        ("traffic class", socket.set_tclass_v6(MEDIA_CLASS)),
    ];
    for (what, result) in tuned {
        if let Err(error) = result {
            tracing::debug!("cannot set the {what}: {error}");
        }
    }
    let address = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0));
    socket.bind(&address.into()).map_err(LinkError::Endpoint)?;
    Ok(socket.into())
}

pub async fn read_message(
    recv: &mut quinn::RecvStream,
) -> Result<Option<(u8, Vec<u8>)>, LinkError> {
    let mut header = [0_u8; HEADER_BYTES];
    match recv.read_exact(&mut header).await {
        Ok(()) => {}
        Err(quinn::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let header = Header::parse(header)?;
    let mut body = vec![0; header.body];
    recv.read_exact(&mut body).await?;
    Ok(Some((header.kind, body)))
}
