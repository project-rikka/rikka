use rikka_coded::Coded;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avutil::{self, AVRational};

pub(crate) const MICROS: AVRational = AVRational {
    num: 1,
    den: 1_000_000,
};

pub(crate) const MUXED: AVRational = AVRational {
    num: 1,
    den: 90_000,
};

pub(crate) fn from_packet(packet: &Packet, time_base: AVRational) -> Option<Coded> {
    let micros = |value: i64| unsafe { avutil::av_rescale_q(value, time_base, MICROS) };
    let pts = packet.pts().map(micros);
    let dts = packet.dts().map(micros);
    let pts_us = pts.or(dts)?;
    Some(Coded {
        data: packet.bytes().into(),
        pts_us,
        dts_us: dts.unwrap_or(pts_us),
        duration_us: micros(packet.span()),
        keyframe: packet.is_key(),
    })
}
