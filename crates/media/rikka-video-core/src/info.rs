use std::path::Path;
use std::time::Duration;

use crate::demux::Demuxer;
use crate::error::{Error, Result};
use crate::init;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: Option<Duration>,
    pub frame_rate: f64,
}

impl VideoInfo {
    pub fn from_demuxer(demuxer: &Demuxer) -> Result<VideoInfo> {
        let parameters = demuxer.stream().codecpar;
        let (width, height) = unsafe { ((*parameters).width, (*parameters).height) };
        if width <= 0 || height <= 0 {
            return Err(Error::Unsupported(format!(
                "stream reports {width}x{height}"
            )));
        }
        Ok(VideoInfo {
            width: width as u32,
            height: height as u32,
            duration: demuxer
                .duration_us()
                .map(|us| Duration::from_micros(us as u64)),
            frame_rate: demuxer.frame_rate(),
        })
    }
}

pub fn probe(path: impl AsRef<Path>) -> Result<VideoInfo> {
    init::ensure()?;
    VideoInfo::from_demuxer(&Demuxer::open(path.as_ref())?)
}
