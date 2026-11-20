use std::sync::Arc;

use rikka_coded::Coded;

pub(crate) const SECOND: i64 = 1_000_000;

pub(crate) fn frame(pts_us: i64, keyframe: bool, bytes: usize) -> Coded {
    Coded {
        data: Arc::from(vec![u8::from(keyframe); bytes]),
        pts_us,
        dts_us: pts_us,
        duration_us: 0,
        keyframe,
    }
}

pub(crate) fn sound(pts_us: i64) -> Coded {
    Coded {
        data: Arc::from(vec![7u8; 16]),
        pts_us,
        dts_us: pts_us,
        duration_us: 20_000,
        keyframe: true,
    }
}

pub(crate) fn pts(packets: &[Coded]) -> Vec<i64> {
    packets.iter().map(|packet| packet.pts_us).collect()
}
