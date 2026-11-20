use std::time::{Duration, SystemTime, UNIX_EPOCH};

const VERSION: u8 = 2;

const SENDER_REPORT: u8 = 200;
const RECEIVER_REPORT: u8 = 201;
const TRANSPORT_FEEDBACK: u8 = 205;
const PAYLOAD_FEEDBACK: u8 = 206;

const GENERIC_NACK: u8 = 1;
const PICTURE_LOSS: u8 = 1;
const FULL_INTRA: u8 = 4;
const APPLICATION: u8 = 15;

const REPORT_BLOCK: usize = 24;
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportBlock {
    pub source: u32,
    pub fraction_lost: u8,
    pub lost: u32,
    pub highest: u32,
    pub jitter: u32,
    pub last_report: u32,
    pub since_report: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feedback {
    Report(Vec<ReportBlock>),
    Sender { source: u32, ntp: u64 },
    Nack { media: u32, lost: Vec<u16> },
    PictureLoss { media: u32 },
    Remb { bitrate: u64, media: Vec<u32> },
}

#[must_use]
pub fn parse_rtcp(compound: &[u8]) -> Vec<Feedback> {
    let mut feedback = Vec::new();
    let mut rest = compound;
    while rest.len() >= 4 {
        let length = (usize::from(u16::from_be_bytes([rest[2], rest[3]])) + 1) * 4;
        let Some(packet) = rest.get(..length) else {
            break;
        };
        if packet[0] >> 6 != VERSION {
            break;
        }
        one(packet, &mut feedback);
        rest = &rest[length..];
    }
    feedback
}

fn one(packet: &[u8], feedback: &mut Vec<Feedback>) {
    let count = packet[0] & 0x1F;
    let body = &packet[4..];
    match packet[1] {
        SENDER_REPORT if body.len() >= 24 => {
            let source = word(body, 0);
            let ntp = u64::from(word(body, 4)) << 32 | u64::from(word(body, 8));
            feedback.push(Feedback::Sender { source, ntp });
            feedback.push(Feedback::Report(blocks(&body[24..], count)));
        }
        RECEIVER_REPORT if body.len() >= 4 => {
            feedback.push(Feedback::Report(blocks(&body[4..], count)));
        }
        TRANSPORT_FEEDBACK if count == GENERIC_NACK && body.len() >= 8 => {
            let media = word(body, 4);
            let mut lost = Vec::new();
            for fci in body[8..].as_chunks::<4>().0 {
                let first = u16::from_be_bytes([fci[0], fci[1]]);
                let mask = u16::from_be_bytes([fci[2], fci[3]]);
                lost.push(first);
                lost.extend(
                    (0..16)
                        .filter(|bit| mask & (1 << bit) != 0)
                        .map(|bit| first.wrapping_add(bit + 1)),
                );
            }
            feedback.push(Feedback::Nack { media, lost });
        }
        PAYLOAD_FEEDBACK if count == PICTURE_LOSS && body.len() >= 8 => {
            feedback.push(Feedback::PictureLoss {
                media: word(body, 4),
            });
        }
        PAYLOAD_FEEDBACK if count == FULL_INTRA && body.len() >= 16 => {
            for fci in body[8..].as_chunks::<8>().0 {
                feedback.push(Feedback::PictureLoss {
                    media: word(fci, 0),
                });
            }
        }
        PAYLOAD_FEEDBACK if count == APPLICATION && body.len() >= 16 && &body[8..12] == b"REMB" => {
            let sources = usize::from(body[12]);
            let exponent = body[13] >> 2;
            let mantissa =
                u64::from(body[13] & 0x03) << 16 | u64::from(body[14]) << 8 | u64::from(body[15]);
            let media = body[16..]
                .as_chunks::<4>()
                .0
                .iter()
                .take(sources)
                .map(|chunk| word(chunk, 0))
                .collect();
            feedback.push(Feedback::Remb {
                bitrate: mantissa
                    .checked_shl(u32::from(exponent))
                    .unwrap_or(u64::MAX),
                media,
            });
        }
        _ => {}
    }
}

fn blocks(body: &[u8], count: u8) -> Vec<ReportBlock> {
    body.as_chunks::<REPORT_BLOCK>()
        .0
        .iter()
        .take(usize::from(count))
        .map(|block| ReportBlock {
            source: word(block, 0),
            fraction_lost: block[4],
            lost: u32::from_be_bytes([0, block[5], block[6], block[7]]),
            highest: word(block, 8),
            jitter: word(block, 12),
            last_report: word(block, 16),
            since_report: word(block, 20),
        })
        .collect()
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderInfo {
    pub source: u32,
    pub ntp: u64,
    pub timestamp: u32,
    pub packets: u32,
    pub octets: u32,
}

#[must_use]
pub fn sender_report(sent: SenderInfo) -> Vec<u8> {
    let mut packet = header(0, SENDER_REPORT, 6);
    packet.extend_from_slice(&sent.source.to_be_bytes());
    packet.extend_from_slice(&sent.ntp.to_be_bytes());
    packet.extend_from_slice(&sent.timestamp.to_be_bytes());
    packet.extend_from_slice(&sent.packets.to_be_bytes());
    packet.extend_from_slice(&sent.octets.to_be_bytes());
    packet
}

#[must_use]
pub fn receiver_report(sender: u32, blocks: &[ReportBlock]) -> Vec<u8> {
    let blocks = &blocks[..blocks.len().min(31)];
    let words = 1 + blocks.len() * REPORT_BLOCK / 4;
    let mut packet = header(blocks.len() as u8, RECEIVER_REPORT, words as u16);
    packet.extend_from_slice(&sender.to_be_bytes());
    for block in blocks {
        packet.extend_from_slice(&block.source.to_be_bytes());
        packet.push(block.fraction_lost);
        packet.extend_from_slice(&block.lost.min(0x00FF_FFFF).to_be_bytes()[1..]);
        packet.extend_from_slice(&block.highest.to_be_bytes());
        packet.extend_from_slice(&block.jitter.to_be_bytes());
        packet.extend_from_slice(&block.last_report.to_be_bytes());
        packet.extend_from_slice(&block.since_report.to_be_bytes());
    }
    packet
}

#[must_use]
pub fn nack(sender: u32, media: u32, lost: &[u16]) -> Option<Vec<u8>> {
    let mut fcis: Vec<(u16, u16)> = Vec::new();
    for &sequence in lost {
        match fcis.last_mut() {
            Some((first, mask)) if sequence.wrapping_sub(*first).wrapping_sub(1) < 16 => {
                *mask |= 1 << sequence.wrapping_sub(*first).wrapping_sub(1);
            }
            _ => fcis.push((sequence, 0)),
        }
    }
    if fcis.is_empty() {
        return None;
    }
    let mut packet = header(GENERIC_NACK, TRANSPORT_FEEDBACK, 2 + fcis.len() as u16);
    packet.extend_from_slice(&sender.to_be_bytes());
    packet.extend_from_slice(&media.to_be_bytes());
    for (first, mask) in fcis {
        packet.extend_from_slice(&first.to_be_bytes());
        packet.extend_from_slice(&mask.to_be_bytes());
    }
    Some(packet)
}

#[must_use]
pub fn picture_loss(sender: u32, media: u32) -> Vec<u8> {
    let mut packet = header(PICTURE_LOSS, PAYLOAD_FEEDBACK, 2);
    packet.extend_from_slice(&sender.to_be_bytes());
    packet.extend_from_slice(&media.to_be_bytes());
    packet
}

fn header(count: u8, kind: u8, words: u16) -> Vec<u8> {
    let mut packet = Vec::with_capacity((usize::from(words) + 1) * 4);
    packet.push(VERSION << 6 | count & 0x1F);
    packet.push(kind);
    packet.extend_from_slice(&words.to_be_bytes());
    packet
}

#[must_use]
pub fn ntp_now() -> u64 {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    ntp(since)
}

pub(crate) fn ntp(since_unix: Duration) -> u64 {
    let seconds = since_unix.as_secs() + NTP_UNIX_OFFSET;
    let fraction = (u64::from(since_unix.subsec_nanos()) << 32) / 1_000_000_000;
    seconds << 32 | fraction
}

pub(crate) fn middle(ntp: u64) -> u32 {
    (ntp >> 16) as u32
}
