use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use discord_rtp::{
    Cipher, DISCOVERY_LEN, Discovered, Feedback, Opened, Plain, SenderInfo, VoiceStream, is_rtcp,
    ntp_now, parse_rtcp,
};
use tokio::net::UdpSocket;
use tokio::time::Instant;

use crate::error::{Error, Result};

const DISCOVERY_ATTEMPTS: u32 = 5;

const DISCOVERY_WAIT: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(crate) enum Arrival {
    Media(Opened),
    Control(Vec<Feedback>),
}

#[derive(Debug)]
pub(crate) struct Media {
    socket: UdpSocket,
    ssrc: u32,
    cipher: Option<Cipher>,
    stream: VoiceStream,
    counter: u32,
    transport: u16,
    keepalives: u32,
    audio_sent: Option<SenderInfo>,
}

impl Media {
    pub(crate) async fn connect(ip: &str, port: u16, ssrc: u32) -> Result<(Media, Discovered)> {
        let address: IpAddr = ip
            .parse()
            .map_err(|_| Error::Unreadable(format!("the voice server address {ip:?}")))?;
        let local = match address {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        };
        let socket = UdpSocket::bind(SocketAddr::new(local, 0))
            .await
            .map_err(Error::unreachable)?;
        socket
            .connect(SocketAddr::new(address, port))
            .await
            .map_err(Error::unreachable)?;
        let discovered = discover(&socket, ssrc).await?;
        let media = Media {
            socket,
            ssrc,
            cipher: None,
            stream: VoiceStream::new(ssrc, Instant::now().into_std()),
            counter: 0,
            transport: 0,
            keepalives: 0,
            audio_sent: None,
        };
        Ok((media, discovered))
    }

    pub(crate) fn ssrc(&self) -> u32 {
        self.ssrc
    }

    pub(crate) fn secure(&mut self, cipher: Cipher) {
        self.cipher = Some(cipher);
    }

    pub(crate) fn is_secure(&self) -> bool {
        self.cipher.is_some()
    }

    pub(crate) fn talkspurt(&mut self) {
        self.stream.talkspurt();
    }

    pub(crate) async fn send(&mut self, frame: &[u8]) {
        let header = self.stream.next(Instant::now().into_std());
        let mut packet = Vec::with_capacity(header.len() + frame.len());
        packet.extend_from_slice(&header);
        packet.extend_from_slice(frame);
        let Some(sealed) = self.seal(&packet) else {
            return;
        };
        let sent = self.audio_sent.get_or_insert(SenderInfo {
            source: self.ssrc,
            ntp: 0,
            timestamp: 0,
            packets: 0,
            octets: 0,
        });
        sent.ntp = ntp_now();
        sent.timestamp = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        sent.packets = sent.packets.wrapping_add(1);
        sent.octets = sent.octets.wrapping_add(frame.len() as u32);
        self.transmit(&sealed).await;
    }

    pub(crate) fn seal_video(&mut self, plain: &Plain) -> Option<Vec<u8>> {
        let packet = plain.packet(self.transport);
        self.transport = self.transport.wrapping_add(1);
        self.seal(&packet)
    }

    pub(crate) fn seal_control(&mut self, rtcp: &[u8]) -> Option<Vec<u8>> {
        let counter = self.next_counter();
        self.cipher.as_ref()?.seal_rtcp(rtcp, counter)
    }

    pub(crate) fn audio_report(&self) -> Option<SenderInfo> {
        self.audio_sent
    }

    pub(crate) async fn transmit(&self, packet: &[u8]) {
        if let Err(error) = self.socket.send(packet).await {
            tracing::debug!(%error, "failed to send voice packet");
        }
    }

    pub(crate) async fn keepalive(&mut self) {
        let mut packet = [0; 8];
        packet[..4].copy_from_slice(&self.keepalives.to_le_bytes());
        self.keepalives = self.keepalives.wrapping_add(1);
        self.transmit(&packet).await;
    }

    pub(crate) async fn receive(&self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.socket.recv(buffer).await
    }

    pub(crate) fn open(&self, datagram: &[u8]) -> Option<Arrival> {
        let cipher = self.cipher.as_ref()?;
        if is_rtcp(datagram) {
            let compound = cipher.open_rtcp(datagram)?;
            return Some(Arrival::Control(parse_rtcp(&compound)));
        }
        let media = cipher.open_rtp(datagram)?;
        (media.header.ssrc != self.ssrc).then_some(Arrival::Media(media))
    }

    fn seal(&mut self, packet: &[u8]) -> Option<Vec<u8>> {
        let counter = self.next_counter();
        self.cipher.as_ref()?.seal_rtp(packet, counter)
    }

    fn next_counter(&mut self) -> u32 {
        let counter = self.counter;
        self.counter = self.counter.wrapping_add(1);
        counter
    }
}

async fn discover(socket: &UdpSocket, ssrc: u32) -> Result<Discovered> {
    let request = Discovered::request(ssrc);
    let mut buffer = [0; 2 * DISCOVERY_LEN];
    for _ in 0..DISCOVERY_ATTEMPTS {
        socket.send(&request).await.map_err(Error::unreachable)?;
        let deadline = Instant::now() + DISCOVERY_WAIT;
        while let Ok(received) = tokio::time::timeout_at(deadline, socket.recv(&mut buffer)).await {
            let length = received.map_err(Error::unreachable)?;
            if let Some(discovered) = Discovered::from_response(&buffer[..length], ssrc) {
                return Ok(discovered);
            }
        }
    }
    Err(Error::Discovery)
}
